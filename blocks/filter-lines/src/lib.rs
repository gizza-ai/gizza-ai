//! gizza-ai/filter-lines — keep or drop lines matching a substring or regex.
//! Chat schema single-sourced from descriptor(); handler delegates to run_skill.
//! Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_filter_lines_core::{filter, Mode};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    pattern: String,
    #[serde(default = "default_mode")]
    mode: String,
    #[serde(default)]
    regex: bool,
    #[serde(default)]
    ignore_case: bool,
}

fn default_mode() -> String {
    "keep".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("text").required().describe("The text whose lines should be filtered."))
        .param(
            Param::string("pattern")
                .required()
                .describe("The substring (or regular expression, if regex=true) to match against each line."),
        )
        .param(
            Param::enumv("mode", ["keep", "drop"])
                .default("keep")
                .describe("keep = output only matching lines; drop = output only non-matching lines."),
        )
        .param(
            Param::boolean("regex")
                .default(false)
                .describe("Treat the pattern as a regular expression instead of a literal substring."),
        )
        .param(
            Param::boolean("ignore_case")
                .default(false)
                .describe("Match case-insensitively."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct FilterLines;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/filter-lines",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Keep or drop lines matching a substring or regex",
    skill(
        description = "Filter text line by line, like grep. Keep (mode=keep, default) or drop (mode=drop) every line that matches a pattern. The pattern is a literal substring by default, or a regular expression when regex=true; set ignore_case=true to match case-insensitively. Returns the filtered text plus total/matched/kept line counts. Runs locally.",
        parameters = schema_json()
    ),
)]
impl FilterLines {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "filter-lines", |a: Args| {
            let mode = Mode::parse(&a.mode).map_err(SkillError::InvalidArgs)?;
            filter(&a.text, &a.pattern, mode, a.regex, a.ignore_case).map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "text": { "type": "string", "description": "The text whose lines should be filtered." },
                    "pattern": { "type": "string", "description": "The substring (or regular expression, if regex=true) to match against each line." },
                    "mode": { "type": "string", "enum": ["keep", "drop"], "default": "keep", "description": "keep = output only matching lines; drop = output only non-matching lines." },
                    "regex": { "type": "boolean", "default": false, "description": "Treat the pattern as a regular expression instead of a literal substring." },
                    "ignore_case": { "type": "boolean", "default": false, "description": "Match case-insensitively." }
                },
                "required": ["text", "pattern"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
