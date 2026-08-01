//! gizza-ai/duplicate-column-detector — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

fn default_true() -> bool { true }

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default = "default_true")]
    header: bool,
    #[serde(default)]
    delimiter: String,
    #[serde(default = "default_true")]
    ignore_case: bool,
    #[serde(default = "default_true")]
    ignore_whitespace: bool,
    #[serde(default = "default_true")]
    ignore_header_name: bool,
    #[serde(default)]
    output: String,
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("data")
                .required()
                .describe("CSV/table text to scan for duplicate columns. Rows may use comma, tab, semicolon, pipe, or a one-character delimiter."),
        )
        .param(
            Param::boolean("header")
                .default(true)
                .describe("When true (default), treat the first row as column names and preserve it in CSV output."),
        )
        .param(
            Param::enumv("delimiter", ["comma", "tab", "semicolon", "pipe"])
                .default("comma")
                .describe("Input delimiter: comma (default), tab, semicolon, or pipe."),
        )
        .param(
            Param::boolean("ignore_case")
                .default(true)
                .describe("When true (default), compare cells case-insensitively so 'Alice' and 'alice' match."),
        )
        .param(
            Param::boolean("ignore_whitespace")
                .default(true)
                .describe("When true (default), trim/collapse whitespace before comparing cells."),
        )
        .param(
            Param::boolean("ignore_header_name")
                .default(true)
                .describe("When true (default), columns with different names still count as duplicates if their values match. Set false to require matching header names too."),
        )
        .param(
            Param::enumv("output", ["report", "csv", "json"])
                .default("report")
                .describe("Output mode: report (human summary, default), csv (table with redundant columns removed), or json (structured duplicate groups)."),
        )
}
fn schema_json() -> String { descriptor().to_schema_json() }

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/duplicate-column-detector",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Find and remove duplicate columns in CSV tables.",
    skill(
        description = "Find duplicate columns in CSV/table text: columns are duplicates when their values match down every row. By default the header name is ignored, so differently named columns with identical values are grouped; set ignore_header_name=false to require matching names too. Comparisons are case-insensitive and whitespace-normalized by default. The first (leftmost) column in each duplicate group is kept, later copies are reported or removed. Choose output='report' for a human summary, 'csv' for the table with redundant columns removed, or 'json' for structured groups.",
        parameters = schema_json()
    )
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "duplicate-column-detector", |a: Args| {
            gizza_ai_duplicate_column_detector_core::detect(
                &a.data,
                a.header,
                &a.delimiter,
                a.ignore_case,
                a.ignore_whitespace,
                a.ignore_header_name,
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
                    "data": { "type": "string", "description": "CSV/table text to scan for duplicate columns. Rows may use comma, tab, semicolon, pipe, or a one-character delimiter." },
                    "header": { "type": "boolean", "default": true, "description": "When true (default), treat the first row as column names and preserve it in CSV output." },
                    "delimiter": { "type": "string", "enum": ["comma", "tab", "semicolon", "pipe"], "default": "comma", "description": "Input delimiter: comma (default), tab, semicolon, or pipe." },
                    "ignore_case": { "type": "boolean", "default": true, "description": "When true (default), compare cells case-insensitively so 'Alice' and 'alice' match." },
                    "ignore_whitespace": { "type": "boolean", "default": true, "description": "When true (default), trim/collapse whitespace before comparing cells." },
                    "ignore_header_name": { "type": "boolean", "default": true, "description": "When true (default), columns with different names still count as duplicates if their values match. Set false to require matching header names too." },
                    "output": { "type": "string", "enum": ["report", "csv", "json"], "default": "report", "description": "Output mode: report (human summary, default), csv (table with redundant columns removed), or json (structured duplicate groups)." }
                },
                "required": ["data"],
                "additionalProperties": false
            }"#,
        ).unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
