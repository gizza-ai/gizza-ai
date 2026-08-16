//! gizza-ai/json-error-locator — pinpoint the line, column and cause of every
//! JSON syntax error in pasted text, with a caret-marked context snippet and a
//! concrete fix. Chat schema single-sourced from descriptor(); handle()
//! delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_json_error_locator_core::locate;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    json: String,
    #[serde(default = "default_output")]
    output: String,
    #[serde(default = "default_context_lines")]
    context_lines: usize,
    #[serde(default = "default_true")]
    scan_all: bool,
}

fn default_output() -> String {
    "report".into()
}
fn default_context_lines() -> usize {
    2
}
fn default_true() -> bool {
    true
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("json").required().describe(
            "The JSON text to diagnose. Paste the whole document, valid or not: valid input returns a summary (top-level type, member count, nesting depth, size) and invalid input returns every syntax problem found with its line, column and character offset. Example: {\"name\": \"Ada\", \"tags\": [1, 2,]}",
        ))
        .param(
            Param::enumv("output", ["report", "json"])
                .default("report")
                .describe("Result format. 'report' (default) is a human-readable listing — one numbered entry per issue with its line/column/offset, a plain-English cause, a suggested fix and a caret-marked source snippet. 'json' returns a machine-readable {valid, issue_count, issues[], parser_stop, summary} document for scripts and CI."),
        )
        .param(
            Param::integer("context_lines")
                .default(2)
                .min(0.0)
                .max(10.0)
                .describe("How many source lines to show above and below each flagged line, with a caret under the exact column (0-10). Set 0 to omit snippets entirely and get just the positions, causes and fixes. Default 2."),
        )
        .param(
            Param::boolean("scan_all")
                .default(true)
                .describe("Report every issue in the document instead of stopping at the first one. A normal JSON parser aborts at the first error, so a file with five mistakes takes five runs to clean up; with scan_all=true a tolerant scanner keeps going and lists them all. Set false to see only the first issue, the way a parser behaves. Default true."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/json-error-locator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Find the line, column and cause of every JSON syntax error",
    skill(
        description = "Locate JSON syntax errors in pasted text and explain them. Valid input returns a summary (top-level type, member count, nesting depth, lines, bytes). Invalid input returns every problem found — not just the first, the way a parser stops — each with a 1-based line, a character-counted column, a 0-based offset, a plain-English cause, a concrete fix, and an optional caret-marked source snippet. Named problems include trailing commas, single-quoted strings and keys, unquoted keys, unquoted bare-word values, missing commas, missing colons, mismatched or unclosed brackets, unterminated strings, invalid \\escapes and short \\u escapes, unescaped control characters, invalid numbers (leading zeros, .5, 1., +1, hex), JavaScript/Python literals (undefined, NaN, Infinity, True, None), // and /* */ comments, and extra content after the document. output picks a human-readable report or a machine-readable JSON document; context_lines sets the snippet size (0-10); scan_all=false reports only the first issue. Input is capped at 1 MiB and nesting at 200 levels. Deterministic and syntax-only — it diagnoses rather than rewrites, and does not validate against a JSON Schema. Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "json-error-locator", |a: Args| {
            locate(&a.json, &a.output, a.context_lines, a.scan_all).map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the authored chat/CLI schema must stay identical to the one
    /// derived from descriptor(). Update BOTH sides together, on purpose.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "json": { "type": "string", "description": "The JSON text to diagnose. Paste the whole document, valid or not: valid input returns a summary (top-level type, member count, nesting depth, size) and invalid input returns every syntax problem found with its line, column and character offset. Example: {\"name\": \"Ada\", \"tags\": [1, 2,]}" },
                    "output": { "type": "string", "enum": ["report", "json"], "default": "report", "description": "Result format. 'report' (default) is a human-readable listing — one numbered entry per issue with its line/column/offset, a plain-English cause, a suggested fix and a caret-marked source snippet. 'json' returns a machine-readable {valid, issue_count, issues[], parser_stop, summary} document for scripts and CI." },
                    "context_lines": { "type": "integer", "default": 2, "minimum": 0, "maximum": 10, "description": "How many source lines to show above and below each flagged line, with a caret under the exact column (0-10). Set 0 to omit snippets entirely and get just the positions, causes and fixes. Default 2." },
                    "scan_all": { "type": "boolean", "default": true, "description": "Report every issue in the document instead of stopping at the first one. A normal JSON parser aborts at the first error, so a file with five mistakes takes five runs to clean up; with scan_all=true a tolerant scanner keeps going and lists them all. Set false to see only the first issue, the way a parser behaves. Default true." }
                },
                "required": ["json"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(authored, derived);
    }
}
