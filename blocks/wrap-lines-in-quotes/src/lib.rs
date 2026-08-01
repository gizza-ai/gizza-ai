//! gizza-ai/wrap-lines-in-quotes — chat skill block on the shared tool abstraction.
//! Wraps each (non-empty) line of text in chosen quotes or brackets, with an
//! optional trailing separator — the classic "turn a column of values into a
//! SQL `IN (…)` list / JSON array / CSV row" helper. The chat schema is
//! single-sourced from descriptor() (which also drives the CLI); handle()
//! delegates to run_skill. Pure → runs on all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_wrap_lines_in_quotes_core::{wrap_lines, WrapStyle};
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default = "default_wrap")]
    wrap: String,
    #[serde(default)]
    open: String,
    #[serde(default)]
    close: String,
    #[serde(default)]
    separator: String,
    #[serde(default)]
    last_line_separator: bool,
    #[serde(default = "default_true")]
    skip_empty: bool,
    #[serde(default)]
    trim: bool,
    #[serde(default)]
    escape: bool,
}

fn default_wrap() -> String {
    "double".to_string()
}
fn default_true() -> bool {
    true
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The text whose lines should each be wrapped (split on newlines)."),
        )
        .param(
            Param::enumv(
                "wrap",
                ["double", "single", "backtick", "paren", "square", "curly", "angle", "guillemet", "custom"],
            )
            .default("double")
            .describe("Delimiter style around each line: double \"…\", single '…', backtick `…`, paren (…), square […], curly {…}, angle <…>, guillemet «…», or custom (uses open/close). Default double."),
        )
        .param(
            Param::string("open")
                .describe("Opening delimiter when wrap=custom (e.g. `<!--`). Ignored for presets."),
        )
        .param(
            Param::string("close")
                .describe("Closing delimiter when wrap=custom. Empty mirrors the opening delimiter. Ignored for presets."),
        )
        .param(
            Param::string("separator")
                .describe("String appended after each wrapped line, e.g. `,` for a SQL IN-list or JSON array. Default none."),
        )
        .param(
            Param::boolean("last_line_separator")
                .default(false)
                .describe("When true, the last wrapped line also gets the trailing separator. Default false (so a comma separator yields a valid IN(…)/array body)."),
        )
        .param(
            Param::boolean("skip_empty")
                .default(true)
                .describe("When true (default), blank/whitespace-only lines pass through unchanged — not wrapped, no separator."),
        )
        .param(
            Param::boolean("trim")
                .default(false)
                .describe("When true, strip surrounding whitespace from each line before wrapping. Default false."),
        )
        .param(
            Param::boolean("escape")
                .default(false)
                .describe("When true, backslash-escape the delimiter chars (and `\\`) inside each line so the output is a valid quoted string. Default false."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/wrap-lines-in-quotes",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Wrap each line of text in quotes or brackets",
    skill(
        description = "Wrap each line of text in chosen quotes or brackets, with an optional trailing separator — great for turning a column of values into a SQL `IN (…)` list, JSON array, or CSV row. wrap picks the delimiter style: double \"…\" (default), single '…', backtick, paren (…), square […], curly {…}, angle <…>, guillemet «…», or custom (then open/close set your own delimiters; an empty close mirrors open). separator is appended after each wrapped line (e.g. `,`); by default the last wrapped line omits it (set last_line_separator=true to keep it). skip_empty (default true) leaves blank lines unchanged. trim strips surrounding whitespace per line; escape backslash-escapes the delimiter inside each line so the result stays valid. Returns the wrapped text plus total/wrapped line counts. Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "wrap-lines-in-quotes", |a: Args| {
            let style = WrapStyle::parse(&a.wrap).map_err(SkillError::InvalidArgs)?;
            let (open, close) = style.delims(&a.open, &a.close).map_err(SkillError::InvalidArgs)?;
            wrap_lines(
                &a.text,
                &open,
                &close,
                &a.separator,
                a.last_line_separator,
                a.skip_empty,
                a.trim,
                a.escape,
            )
            .map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema, so any future change to the LLM-facing API is intentional.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "text": { "type": "string", "description": "The text whose lines should each be wrapped (split on newlines)." },
                    "wrap": { "type": "string", "enum": ["double", "single", "backtick", "paren", "square", "curly", "angle", "guillemet", "custom"], "default": "double", "description": "Delimiter style around each line: double \"…\", single '…', backtick `…`, paren (…), square […], curly {…}, angle <…>, guillemet «…», or custom (uses open/close). Default double." },
                    "open": { "type": "string", "description": "Opening delimiter when wrap=custom (e.g. `<!--`). Ignored for presets." },
                    "close": { "type": "string", "description": "Closing delimiter when wrap=custom. Empty mirrors the opening delimiter. Ignored for presets." },
                    "separator": { "type": "string", "description": "String appended after each wrapped line, e.g. `,` for a SQL IN-list or JSON array. Default none." },
                    "last_line_separator": { "type": "boolean", "default": false, "description": "When true, the last wrapped line also gets the trailing separator. Default false (so a comma separator yields a valid IN(…)/array body)." },
                    "skip_empty": { "type": "boolean", "default": true, "description": "When true (default), blank/whitespace-only lines pass through unchanged — not wrapped, no separator." },
                    "trim": { "type": "boolean", "default": false, "description": "When true, strip surrounding whitespace from each line before wrapping. Default false." },
                    "escape": { "type": "boolean", "default": false, "description": "When true, backslash-escape the delimiter chars (and `\\`) inside each line so the output is a valid quoted string. Default false." }
                },
                "required": ["text"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
