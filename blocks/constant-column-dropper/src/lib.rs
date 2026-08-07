//! gizza-ai/constant-column-dropper — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

fn default_true() -> bool { true }
fn default_dominance() -> f64 { 100.0 }

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default = "default_true")]
    header: bool,
    #[serde(default)]
    delimiter: String,
    #[serde(default = "default_dominance")]
    dominance: f64,
    #[serde(default)]
    empty_cells: String,
    #[serde(default = "default_true")]
    ignore_case: bool,
    #[serde(default = "default_true")]
    ignore_whitespace: bool,
    #[serde(default)]
    keep: String,
    #[serde(default)]
    output: String,
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("data")
                .required()
                .describe("CSV/table text to scan for constant (zero-variance) columns, one row per line. Example: 'id,country,score\\n1,US,10\\n2,US,20'."),
        )
        .param(
            Param::boolean("header")
                .default(true)
                .describe("When true (default), treat the first row as column names: it is excluded from the value counts and preserved in CSV output."),
        )
        .param(
            Param::enumv("delimiter", ["comma", "tab", "semicolon", "pipe"])
                .default("comma")
                .describe("Input field separator: comma (default), tab, semicolon, or pipe. CSV output uses the same separator."),
        )
        .param(
            Param::number("dominance")
                .min(50.0)
                .max(100.0)
                .default(100.0)
                .describe("Drop a column once its most frequent value covers at least this percent of the counted rows. 100 (default) drops only strictly constant columns; 95 also drops near-constant ones (95% of rows the same). Range 50-100."),
        )
        .param(
            Param::enumv("empty_cells", ["value", "ignore"])
                .default("value")
                .describe("How empty cells count: 'value' (default) treats an empty cell as its own value, so a column of values plus blanks is not constant; 'ignore' skips empty cells before counting. A column that is entirely empty is dropped either way."),
        )
        .param(
            Param::boolean("ignore_case")
                .default(true)
                .describe("When true (default), compare cells case-insensitively, so a column of 'YES'/'yes' counts as constant."),
        )
        .param(
            Param::boolean("ignore_whitespace")
                .default(true)
                .describe("When true (default), trim and collapse whitespace before comparing, so 'US' and ' US ' count as the same value."),
        )
        .param(
            Param::string("keep")
                .default("")
                .describe("Comma-separated column names or 1-based column numbers that must never be dropped, even when constant. Example: 'id,country' or '1,2'. Empty (default) protects nothing."),
        )
        .param(
            Param::enumv("output", ["report", "csv", "json"])
                .default("report")
                .describe("Output mode: report (human summary of which columns are constant, default), csv (the table with the constant columns removed), or json (per-column metrics: distinct values, top value, top share)."),
        )
}
fn schema_json() -> String { descriptor().to_schema_json() }

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/constant-column-dropper",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Find and remove constant (zero-variance) columns in CSV tables.",
    skill(
        description = "Detect and remove zero-variance columns in CSV/table text: columns holding a single repeated value down every data row, or that are entirely empty. Constancy is measured as one distinct value (works on text, not just numbers). Set dominance below 100 to also catch near-constant columns, e.g. 95 drops a column whose top value covers 95% of rows. empty_cells decides whether a blank counts as its own value or is skipped; keep protects named or numbered columns from being dropped. Choose output='report' for a human summary, 'csv' for the cleaned table, or 'json' for per-column metrics.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "constant-column-dropper", |a: Args| {
            gizza_ai_constant_column_dropper_core::drop_constant(
                &a.data,
                a.header,
                &a.delimiter,
                a.dominance,
                &a.empty_cells,
                a.ignore_case,
                a.ignore_whitespace,
                &a.keep,
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
                    "data": { "type": "string", "description": "CSV/table text to scan for constant (zero-variance) columns, one row per line. Example: 'id,country,score\\n1,US,10\\n2,US,20'." },
                    "header": { "type": "boolean", "default": true, "description": "When true (default), treat the first row as column names: it is excluded from the value counts and preserved in CSV output." },
                    "delimiter": { "type": "string", "enum": ["comma", "tab", "semicolon", "pipe"], "default": "comma", "description": "Input field separator: comma (default), tab, semicolon, or pipe. CSV output uses the same separator." },
                    "dominance": { "type": "number", "minimum": 50, "maximum": 100, "default": 100.0, "description": "Drop a column once its most frequent value covers at least this percent of the counted rows. 100 (default) drops only strictly constant columns; 95 also drops near-constant ones (95% of rows the same). Range 50-100." },
                    "empty_cells": { "type": "string", "enum": ["value", "ignore"], "default": "value", "description": "How empty cells count: 'value' (default) treats an empty cell as its own value, so a column of values plus blanks is not constant; 'ignore' skips empty cells before counting. A column that is entirely empty is dropped either way." },
                    "ignore_case": { "type": "boolean", "default": true, "description": "When true (default), compare cells case-insensitively, so a column of 'YES'/'yes' counts as constant." },
                    "ignore_whitespace": { "type": "boolean", "default": true, "description": "When true (default), trim and collapse whitespace before comparing, so 'US' and ' US ' count as the same value." },
                    "keep": { "type": "string", "default": "", "description": "Comma-separated column names or 1-based column numbers that must never be dropped, even when constant. Example: 'id,country' or '1,2'. Empty (default) protects nothing." },
                    "output": { "type": "string", "enum": ["report", "csv", "json"], "default": "report", "description": "Output mode: report (human summary of which columns are constant, default), csv (the table with the constant columns removed), or json (per-column metrics: distinct values, top value, top share)." }
                },
                "required": ["data"],
                "additionalProperties": false
            }"#,
        ).unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
