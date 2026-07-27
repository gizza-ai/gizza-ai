//! gizza-ai/missing-value-report — chat skill block on the shared tool abstraction.
//! For a CSV/table with a header row, report the count and percentage of missing
//! (blank or NA-token) cells per column plus a missingness-pattern grid (which
//! columns go missing together, the R `mice::md.pattern` idiom). The chat schema
//! is single-sourced from `descriptor()` (which also drives the CLI); `handle()`
//! delegates to `block_utils::run_skill`. Pure compute — nothing is uploaded.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default)]
    delimiter: String,
    #[serde(default = "default_na_values")]
    na_values: String,
    #[serde(default)]
    sort: String,
    #[serde(default = "default_include_patterns")]
    include_patterns: bool,
    #[serde(default = "default_max_patterns")]
    max_patterns: i64,
}

fn default_na_values() -> String {
    "NA,N/A,null,NaN,None,#N/A".to_string()
}
fn default_include_patterns() -> bool {
    true
}
fn default_max_patterns() -> i64 {
    10
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("The CSV/delimited table to profile, as text. The first row MUST be the header; every following row is a data row. Quoted fields with embedded commas/newlines (RFC 4180) are handled."),
        )
        .param(
            Param::string("delimiter")
                .default(",")
                .describe("Field separator: a single character or a name ('comma' (default), 'tab', 'semicolon', 'pipe'). Use 'tab' for TSV data."),
        )
        .param(
            Param::string("na_values")
                .default("NA,N/A,null,NaN,None,#N/A")
                .describe("Comma-separated tokens counted as missing in addition to blank/whitespace-only cells, matched case-insensitively after trimming. Default 'NA,N/A,null,NaN,None,#N/A'. Set empty to count ONLY blank cells as missing."),
        )
        .param(
            Param::enumv("sort", ["missing", "column", "name"])
                .default("missing")
                .describe("Order of the per-column table: 'missing' (default, most-missing-first), 'column' (original header order), or 'name' (alphabetical by column name)."),
        )
        .param(
            Param::boolean("include_patterns")
                .default(true)
                .describe("When true (default), append a missingness-pattern grid: each distinct present(1)/missing(0) combination across the columns with how many rows share it, most-common-first (the R mice::md.pattern idiom)."),
        )
        .param(
            Param::integer("max_patterns")
                .min(1.0)
                .max(1000.0)
                .default(10)
                .describe("Maximum number of pattern rows to list in the grid (1-1000, default 10). Extra patterns are summarized as a '(N more…)' note. Only used when include_patterns is true."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/missing-value-report",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Profile a CSV/table for missing values: per-column missing counts and percentages plus a missingness-pattern grid.",
    skill(
        description = "Profile missing (blank or NA-token) values in a CSV/delimited table. Paste the table as text with a header row; it reports, for every column, how many cells are missing, how many are present, the total, and the missing percentage, plus total rows and how many rows are complete (no missing). When include_patterns is on (default) it also shows a missingness-pattern grid — each distinct present(1)/missing(0) combination and how many rows share it, most-common-first (the R mice::md.pattern idiom) — so you can see which columns tend to go missing together. delimiter accepts a single char or 'comma'/'tab'/'semicolon'/'pipe' (default comma). na_values is a case-insensitive comma-separated list of extra missing tokens (default 'NA,N/A,null,NaN,None,#N/A'; empty = blanks only). sort orders the column table by 'missing' (default), 'column', or 'name'. max_patterns caps the grid rows (default 10). Runs entirely in the sandbox; nothing is uploaded.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "missing-value-report", |a: Args| {
            gizza_ai_missing_value_report_core::run(
                &a.input,
                &a.delimiter,
                &a.na_values,
                &a.sort,
                a.include_patterns,
                a.max_patterns,
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
    /// reviewed. Authored 2026-07-27 for the initial missing-value-report release.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "input": { "type": "string", "description": "The CSV/delimited table to profile, as text. The first row MUST be the header; every following row is a data row. Quoted fields with embedded commas/newlines (RFC 4180) are handled." },
                    "delimiter": { "type": "string", "default": ",", "description": "Field separator: a single character or a name ('comma' (default), 'tab', 'semicolon', 'pipe'). Use 'tab' for TSV data." },
                    "na_values": { "type": "string", "default": "NA,N/A,null,NaN,None,#N/A", "description": "Comma-separated tokens counted as missing in addition to blank/whitespace-only cells, matched case-insensitively after trimming. Default 'NA,N/A,null,NaN,None,#N/A'. Set empty to count ONLY blank cells as missing." },
                    "sort": { "type": "string", "enum": ["missing", "column", "name"], "default": "missing", "description": "Order of the per-column table: 'missing' (default, most-missing-first), 'column' (original header order), or 'name' (alphabetical by column name)." },
                    "include_patterns": { "type": "boolean", "default": true, "description": "When true (default), append a missingness-pattern grid: each distinct present(1)/missing(0) combination across the columns with how many rows share it, most-common-first (the R mice::md.pattern idiom)." },
                    "max_patterns": { "type": "integer", "minimum": 1, "maximum": 1000, "default": 10, "description": "Maximum number of pattern rows to list in the grid (1-1000, default 10). Extra patterns are summarized as a '(N more…)' note. Only used when include_patterns is true." }
                },
                "required": ["input"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
