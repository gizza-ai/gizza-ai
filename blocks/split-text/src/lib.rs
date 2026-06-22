//! gizza-ai/split-text — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. No host calls — runs
//! entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default = "default_delimiter")]
    delimiter: String,
    #[serde(default)]
    mode: String,
    #[serde(default)]
    trim: bool,
    #[serde(default)]
    remove_empty: bool,
}

fn default_delimiter() -> String {
    ",".to_string()
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The text to split."),
        )
        .param(
            Param::string("delimiter")
                .default(",")
                .describe("The substring to split on (default ','). Escapes are recognised: \\n (newline), \\t (tab), \\r, and \\\\ (a literal backslash). Ignored when mode is 'whitespace' or 'chars'."),
        )
        .param(
            Param::enumv("mode", ["literal", "whitespace", "chars"])
                .default("literal")
                .describe("How to split. 'literal' (default) splits on each occurrence of the delimiter; 'whitespace' splits on runs of any whitespace (delimiter ignored); 'chars' emits one character per line (delimiter ignored)."),
        )
        .param(
            Param::boolean("trim")
                .default(false)
                .describe("Trim leading/trailing whitespace from each item. Default false."),
        )
        .param(
            Param::boolean("remove_empty")
                .default(false)
                .describe("Drop empty items (after trimming, if trim is on) — useful for trailing delimiters or blank fields. Default false."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/split-text",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Split Text skill",
    skill(
        description = "Split text on a chosen delimiter into one item per line. By default splits on a comma; set delimiter to any substring (escapes \\n, \\t, \\r, \\\\ are recognised). Use mode='whitespace' to split on runs of spaces/tabs/newlines, or mode='chars' to put each character on its own line. Set trim=true to strip surrounding whitespace from each item and remove_empty=true to drop blank items (e.g. from a trailing delimiter). Returns the items joined by newlines.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "split-text", |a: Args| {
            gizza_ai_split_text_core::split(
                &a.text,
                &a.delimiter,
                &a.mode,
                a.trim,
                a.remove_empty,
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
    /// schema, so any future change to the LLM-facing API is intentional and
    /// reviewed.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "text": { "type": "string", "description": "The text to split." },
                    "delimiter": { "type": "string", "default": ",", "description": "The substring to split on (default ','). Escapes are recognised: \\n (newline), \\t (tab), \\r, and \\\\ (a literal backslash). Ignored when mode is 'whitespace' or 'chars'." },
                    "mode": { "type": "string", "enum": ["literal", "whitespace", "chars"], "default": "literal", "description": "How to split. 'literal' (default) splits on each occurrence of the delimiter; 'whitespace' splits on runs of any whitespace (delimiter ignored); 'chars' emits one character per line (delimiter ignored)." },
                    "trim": { "type": "boolean", "default": false, "description": "Trim leading/trailing whitespace from each item. Default false." },
                    "remove_empty": { "type": "boolean", "default": false, "description": "Drop empty items (after trimming, if trim is on) — useful for trailing delimiters or blank fields. Default false." }
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
