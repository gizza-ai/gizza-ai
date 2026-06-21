//! gizza-ai/regex-extract — return every match of a regular expression found in
//! the input text. Chat schema single-sourced from descriptor(); handler
//! delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_regex_extract_core::extract;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    pattern: String,
    #[serde(default)]
    ignore_case: bool,
    #[serde(default)]
    multiline: bool,
    #[serde(default)]
    dotall: bool,
    #[serde(default)]
    capture_group: f64,
    #[serde(default)]
    unique: bool,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The text to search for matches."),
        )
        .param(
            Param::string("pattern")
                .required()
                .describe("The regular expression (Rust regex syntax) to match against the text."),
        )
        .param(
            Param::boolean("ignore_case")
                .default(false)
                .describe("Match case-insensitively."),
        )
        .param(
            Param::boolean("multiline")
                .default(false)
                .describe(
                    "Let ^ and $ match at line boundaries, not only the start/end of the text.",
                ),
        )
        .param(
            Param::boolean("dotall")
                .default(false)
                .describe("Let . also match newline characters."),
        )
        .param(
            Param::integer("capture_group")
                .default(0)
                .min(0.0)
                .describe("Which capture group to return per match (0 = the whole match)."),
        )
        .param(
            Param::boolean("unique")
                .default(false)
                .describe("Deduplicate the returned matches, keeping first-seen order."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct RegexExtract;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/regex-extract",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Return every match of a regular expression in text",
    skill(
        description = "Search text with a regular expression (Rust regex syntax) and return every match. Toggle ignore_case for case-insensitive matching, multiline so ^/$ anchor per line, and dotall so . spans newlines. Set capture_group to extract a specific captured sub-group instead of the whole match (0 = whole match), and unique to deduplicate the results in first-seen order. Returns the match count and the list of matches. Runs locally.",
        parameters = schema_json()
    ),
)]
impl RegexExtract {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "regex-extract", |a: Args| {
            extract(
                &a.text,
                &a.pattern,
                a.ignore_case,
                a.multiline,
                a.dotall,
                a.capture_group.max(0.0) as usize,
                a.unique,
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

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "text": { "type": "string", "description": "The text to search for matches." },
                    "pattern": { "type": "string", "description": "The regular expression (Rust regex syntax) to match against the text." },
                    "ignore_case": { "type": "boolean", "default": false, "description": "Match case-insensitively." },
                    "multiline": { "type": "boolean", "default": false, "description": "Let ^ and $ match at line boundaries, not only the start/end of the text." },
                    "dotall": { "type": "boolean", "default": false, "description": "Let . also match newline characters." },
                    "capture_group": { "type": "integer", "minimum": 0, "default": 0, "description": "Which capture group to return per match (0 = the whole match)." },
                    "unique": { "type": "boolean", "default": false, "description": "Deduplicate the returned matches, keeping first-seen order." }
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
