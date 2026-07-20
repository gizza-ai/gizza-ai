//! gizza-ai/regex-to-json — parse each line of text with a named-capture regex
//! and emit structured JSON objects keyed by group name. Chat schema
//! single-sourced from descriptor(); handle() delegates to run_skill.
//! Pure → runs on all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_regex_to_json_core::to_json;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    pattern: String,
    #[serde(default)]
    ignore_case: bool,
    #[serde(default)]
    all_matches: bool,
    #[serde(default = "default_unmatched")]
    unmatched: String,
    #[serde(default)]
    coerce_types: bool,
    #[serde(default = "default_output")]
    output: String,
}

fn default_unmatched() -> String {
    "skip".to_string()
}
fn default_output() -> String {
    "json".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The text to parse, one record per line (log lines, exports, command output). Blank lines are ignored; CRLF line endings are handled. Max 1 MB."),
        )
        .param(
            Param::string("pattern")
                .required()
                .describe("Regular expression (Rust regex syntax) with NAMED capture groups — (?<name>…) or (?P<name>…). Each named group becomes a JSON key, e.g. (?<level>[A-Z]+) (?<message>.*). Numbered groups are ignored."),
        )
        .param(
            Param::boolean("ignore_case")
                .default(false)
                .describe("Match case-insensitively."),
        )
        .param(
            Param::boolean("all_matches")
                .default(false)
                .describe("Emit one JSON object for every match on a line instead of only the first — a line with three matches yields three objects (useful for key=value pairs)."),
        )
        .param(
            Param::enumv("unmatched", ["skip", "keep", "fail"])
                .default("skip")
                .describe("What to do with non-blank lines the pattern does not match: 'skip' drops them, 'keep' emits {\"_raw\": \"<line>\"} so no data is lost, 'fail' stops with an error naming the first unmatched line."),
        )
        .param(
            Param::boolean("coerce_types")
                .default(false)
                .describe("Convert captures that look like plain numbers (42, -3.14), true/false, or null into real JSON types instead of strings. Values with leading zeros (007) or scientific notation stay strings."),
        )
        .param(
            Param::enumv("output", ["json", "compact", "ndjson"])
                .default("json")
                .describe("Output shape: 'json' = pretty-printed JSON array, 'compact' = single-line JSON array, 'ndjson' = one JSON object per line (JSON Lines)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/regex-to-json",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Parse lines of text with a named-capture regex into JSON objects",
    skill(
        description = "Parse each line of text with a regular expression that uses NAMED capture groups ((?<name>…) or (?P<name>…), Rust regex syntax) and emit one structured JSON object per line (or per match, with all_matches), keyed by group name in pattern order. Choose what happens to non-matching lines via unmatched (skip them, keep them as {\"_raw\": line}, or fail), optionally coerce number/boolean/null-looking captures into real JSON types with coerce_types, and pick the output shape with output (pretty JSON array, compact array, or NDJSON). Groups that did not participate in a match are emitted as null. Blank lines are ignored; input is capped at 1 MB. Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "regex-to-json", |a: Args| {
            to_json(
                &a.text,
                &a.pattern,
                a.ignore_case,
                a.all_matches,
                &a.unmatched,
                a.coerce_types,
                &a.output,
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
                    "text": { "type": "string", "description": "The text to parse, one record per line (log lines, exports, command output). Blank lines are ignored; CRLF line endings are handled. Max 1 MB." },
                    "pattern": { "type": "string", "description": "Regular expression (Rust regex syntax) with NAMED capture groups — (?<name>…) or (?P<name>…). Each named group becomes a JSON key, e.g. (?<level>[A-Z]+) (?<message>.*). Numbered groups are ignored." },
                    "ignore_case": { "type": "boolean", "default": false, "description": "Match case-insensitively." },
                    "all_matches": { "type": "boolean", "default": false, "description": "Emit one JSON object for every match on a line instead of only the first — a line with three matches yields three objects (useful for key=value pairs)." },
                    "unmatched": { "type": "string", "enum": ["skip", "keep", "fail"], "default": "skip", "description": "What to do with non-blank lines the pattern does not match: 'skip' drops them, 'keep' emits {\"_raw\": \"<line>\"} so no data is lost, 'fail' stops with an error naming the first unmatched line." },
                    "coerce_types": { "type": "boolean", "default": false, "description": "Convert captures that look like plain numbers (42, -3.14), true/false, or null into real JSON types instead of strings. Values with leading zeros (007) or scientific notation stay strings." },
                    "output": { "type": "string", "enum": ["json", "compact", "ndjson"], "default": "json", "description": "Output shape: 'json' = pretty-printed JSON array, 'compact' = single-line JSON array, 'ndjson' = one JSON object per line (JSON Lines)." }
                },
                "required": ["text", "pattern"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn args_defaults_apply() {
        let a: Args = serde_json::from_str(r#"{"text":"a=1","pattern":"(?<k>\\w+)=(?<v>\\d+)"}"#)
            .unwrap();
        assert!(!a.ignore_case);
        assert!(!a.all_matches);
        assert_eq!(a.unmatched, "skip");
        assert!(!a.coerce_types);
        assert_eq!(a.output, "json");
        let out = to_json(
            &a.text,
            &a.pattern,
            a.ignore_case,
            a.all_matches,
            &a.unmatched,
            a.coerce_types,
            &a.output,
        )
        .unwrap();
        assert!(out.contains("\"k\": \"a\""));
    }
}
