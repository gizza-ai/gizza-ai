//! gizza-ai/regex-tester — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill and returns the structured
//! match report (positions + every numbered/named capture group).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_regex_tester_core::test;
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
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The input text to test the regular expression against."),
        )
        .param(
            Param::string("pattern")
                .required()
                .describe("The regular expression (Rust regex syntax) to test."),
        )
        .param(
            Param::boolean("ignore_case")
                .default(false)
                .describe("Match case-insensitively (the i flag)."),
        )
        .param(
            Param::boolean("multiline")
                .default(false)
                .describe("Let ^ and $ match at line boundaries, not only the start/end of the text (the m flag)."),
        )
        .param(
            Param::boolean("dotall")
                .default(false)
                .describe("Let . also match newline characters (the s flag)."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/regex-tester",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Test a regular expression and inspect every match and capture group",
    skill(
        description = "Test a regular expression (Rust regex syntax) against input text and get a full structured breakdown for debugging: every match with its character start/end positions, and within each match the value and span of every capture group — both numbered and named (?<name>…). Toggle ignore_case for case-insensitive matching, multiline so ^/$ anchor per line, and dotall so . spans newlines. Returns the match count, the pattern's capture-group count and names, and the list of matches. Reports an error for an invalid pattern. Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "regex-tester", |a: Args| {
            test(&a.text, &a.pattern, a.ignore_case, a.multiline, a.dotall)
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
                    "text": { "type": "string", "description": "The input text to test the regular expression against." },
                    "pattern": { "type": "string", "description": "The regular expression (Rust regex syntax) to test." },
                    "ignore_case": { "type": "boolean", "default": false, "description": "Match case-insensitively (the i flag)." },
                    "multiline": { "type": "boolean", "default": false, "description": "Let ^ and $ match at line boundaries, not only the start/end of the text (the m flag)." },
                    "dotall": { "type": "boolean", "default": false, "description": "Let . also match newline characters (the s flag)." }
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
