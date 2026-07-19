//! gizza-ai/regex-search — grep-style line search over text. Chat schema
//! single-sourced from descriptor(); handler delegates to run_skill and returns
//! the grep-style rendering. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_regex_search_core::render;
use serde::Deserialize;
use wafer_sdk::*;

fn default_true() -> bool {
    true
}

fn default_zero() -> String {
    "0".to_string()
}

#[derive(Deserialize)]
struct Args {
    text: String,
    pattern: String,
    #[serde(default)]
    literal: bool,
    #[serde(default)]
    ignore_case: bool,
    #[serde(default)]
    whole_word: bool,
    #[serde(default)]
    invert: bool,
    #[serde(default = "default_zero")]
    context: String,
    #[serde(default = "default_true")]
    line_numbers: bool,
    #[serde(default)]
    highlight: bool,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The text to search, line by line."),
        )
        .param(
            Param::string("pattern")
                .required()
                .describe("The pattern to search for: a regular expression (Rust regex syntax), or a literal string when 'literal' is on."),
        )
        .param(
            Param::boolean("literal")
                .default(false)
                .describe("Treat the pattern as a literal fixed string (escape regex metacharacters) instead of a regular expression."),
        )
        .param(
            Param::boolean("ignore_case")
                .default(false)
                .describe("Match case-insensitively."),
        )
        .param(
            Param::boolean("whole_word")
                .default(false)
                .describe("Only match when the pattern forms a whole word (bounded by word boundaries)."),
        )
        .param(
            Param::boolean("invert")
                .default(false)
                .describe("Return the lines that do NOT match instead (like grep -v)."),
        )
        .param(
            Param::string("context")
                .default("0")
                .describe("Also include this many lines of context before and after each matching line (like grep -C). Use a non-negative integer; default 0."),
        )
        .param(
            Param::boolean("line_numbers")
                .default(true)
                .describe("Prefix each line with its 1-based line number (':' for a match, '-' for a context line)."),
        )
        .param(
            Param::boolean("highlight")
                .default(false)
                .describe("Wrap each matched substring on a matching line in angle-quote markers."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/regex-search",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Grep-style search: return the text lines matching a regex or literal pattern",
    skill(
        description = "Search text line by line and return the whole lines that match a pattern, grep-style. The pattern is a regular expression (Rust regex syntax) by default, or a literal fixed string when literal is on. Toggle ignore_case for case-insensitive matching, whole_word to match only complete words, and invert to return the NON-matching lines instead. Set context to include that many lines before and after each match, line_numbers to prefix lines with their number, and highlight to wrap matched substrings in angle-quote markers. Returns a grep-style listing with a count of matching lines. Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "regex-search", |a: Args| {
            render(
                &a.text,
                &a.pattern,
                a.literal,
                a.ignore_case,
                a.whole_word,
                a.invert,
                a.context.trim().parse::<usize>().unwrap_or(0),
                a.line_numbers,
                a.highlight,
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
                    "text": { "type": "string", "description": "The text to search, line by line." },
                    "pattern": { "type": "string", "description": "The pattern to search for: a regular expression (Rust regex syntax), or a literal string when 'literal' is on." },
                    "literal": { "type": "boolean", "default": false, "description": "Treat the pattern as a literal fixed string (escape regex metacharacters) instead of a regular expression." },
                    "ignore_case": { "type": "boolean", "default": false, "description": "Match case-insensitively." },
                    "whole_word": { "type": "boolean", "default": false, "description": "Only match when the pattern forms a whole word (bounded by word boundaries)." },
                    "invert": { "type": "boolean", "default": false, "description": "Return the lines that do NOT match instead (like grep -v)." },
                    "context": { "type": "string", "default": "0", "description": "Also include this many lines of context before and after each matching line (like grep -C). Use a non-negative integer; default 0." },
                    "line_numbers": { "type": "boolean", "default": true, "description": "Prefix each line with its 1-based line number (':' for a match, '-' for a context line)." },
                    "highlight": { "type": "boolean", "default": false, "description": "Wrap each matched substring on a matching line in angle-quote markers." }
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
