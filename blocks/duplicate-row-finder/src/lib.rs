//! gizza-ai/duplicate-row-finder — find (and report) duplicate rows in a table.
//! Thin wrapper around the core; chat schema single-sourced from descriptor();
//! handler delegates to run_skill. Pure. Read-only — reports dupes, never
//! rewrites the data (that's csv-dedupe / fuzzy-dedupe).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_duplicate_row_finder_core::find;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default)]
    columns: String,
    #[serde(default = "default_true")]
    header: bool,
    #[serde(default)]
    delimiter: String,
    #[serde(default = "default_true")]
    ignore_case: bool,
    #[serde(default = "default_true")]
    ignore_whitespace: bool,
    #[serde(default)]
    output: String,
}
fn default_true() -> bool { true }

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe("The table text to scan, e.g. CSV rows. One row per line."))
        .param(Param::string("columns").default("").describe("Key columns to match on: comma-separated header names (needs header=true) or 1-based indices (e.g. 'email' or '1,3'). Blank = match the whole row."))
        .param(Param::boolean("header").default(true).describe("Treat the first row as a header (used to name columns and excluded from the row scan). Default true."))
        .param(Param::string("delimiter").default(",").describe("Field separator: a single char or 'comma'/'tab'/'semicolon'/'pipe'. Default ','."))
        .param(Param::boolean("ignore_case").default(true).describe("Compare case-insensitively so 'Alice' and 'alice' count as the same. Default true."))
        .param(Param::boolean("ignore_whitespace").default(true).describe("Trim and collapse whitespace before comparing so 'New  York ' matches 'New York'. Default true."))
        .param(Param::enumv("output", ["report", "csv", "json"]).default("report").describe("Result format: report (human-readable summary + which columns drive duplication), csv (only the duplicate rows), or json (structured groups + per-column stats). Default report."))
}

fn schema_json() -> String { descriptor().to_schema_json() }

#[cfg(target_arch = "wasm32")]
struct DuplicateRowFinder;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/duplicate-row-finder",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Find and report duplicate rows in a table",
    skill(
        description = "Find duplicate rows in a CSV/delimited table and report them (read-only — it does NOT remove or rewrite rows; use csv-dedupe/fuzzy-dedupe for that). A row is a duplicate when it matches an earlier row on the chosen `columns` (1-based indices or header names) or, when blank, on the whole row. `ignore_case`/`ignore_whitespace` catch near-duplicates that differ only by casing or spacing. It groups the matches with their line numbers and profiles which columns drive the duplication. `output` = report (summary), csv (just the duplicate rows), or json (structured).",
        parameters = schema_json()
    ),
)]
impl DuplicateRowFinder {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "duplicate-row-finder", |a: Args| {
            let delim = if a.delimiter.is_empty() { ",".to_string() } else { a.delimiter };
            let output = if a.output.is_empty() { "report".to_string() } else { a.output };
            find(&a.data, &a.columns, a.header, &delim, a.ignore_case, a.ignore_whitespace, &output)
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
                    "data":              { "type": "string", "description": "The table text to scan, e.g. CSV rows. One row per line." },
                    "columns":           { "type": "string", "default": "", "description": "Key columns to match on: comma-separated header names (needs header=true) or 1-based indices (e.g. 'email' or '1,3'). Blank = match the whole row." },
                    "header":            { "type": "boolean", "default": true, "description": "Treat the first row as a header (used to name columns and excluded from the row scan). Default true." },
                    "delimiter":         { "type": "string", "default": ",", "description": "Field separator: a single char or 'comma'/'tab'/'semicolon'/'pipe'. Default ','." },
                    "ignore_case":       { "type": "boolean", "default": true, "description": "Compare case-insensitively so 'Alice' and 'alice' count as the same. Default true." },
                    "ignore_whitespace": { "type": "boolean", "default": true, "description": "Trim and collapse whitespace before comparing so 'New  York ' matches 'New York'. Default true." },
                    "output":            { "type": "string", "enum": ["report", "csv", "json"], "default": "report", "description": "Result format: report (human-readable summary + which columns drive duplication), csv (only the duplicate rows), or json (structured groups + per-column stats). Default report." }
                },
                "required": ["data"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
