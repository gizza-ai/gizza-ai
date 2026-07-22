//! gizza-ai/csv-column-type-validator — chat skill block on the shared tool abstraction.
//! Validates declared CSV column types and reports offending cells.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

fn default_true() -> bool { true }
fn default_max_issues() -> u64 { 50 }

#[derive(Deserialize)]
struct Args {
    data: String,
    types: String,
    #[serde(default = "default_true")]
    header: bool,
    #[serde(default)]
    delimiter: String,
    #[serde(default)]
    date_format: String,
    #[serde(default = "default_true")]
    empty_ok: bool,
    #[serde(default = "default_max_issues")]
    max_issues: u64,
    #[serde(default)]
    format: String,
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("data")
                .required()
                .describe("CSV data to validate. Paste rows as text; quoted fields and quoted newlines are supported. The first row is treated as a header by default."),
        )
        .param(
            Param::string("types")
                .required()
                .describe("Column type declarations, comma- or newline-separated. Use `column:type` with header names (for example `age:int, joined:date, plan:enum(free|pro)`) or 1-based indexes when header=false (for example `2:int`). Supported types: int, float, bool, date, enum(a|b|c)."),
        )
        .param(
            Param::boolean("header")
                .default(true)
                .describe("Treat the first row as a header row and resolve declarations by column name. Default true. Set false to reference columns by 1-based index in `types`."),
        )
        .param(
            Param::enumv("delimiter", ["auto", "comma", "tab", "semicolon", "pipe"])
                .default("auto")
                .describe("CSV delimiter. `auto` (default) detects comma, tab, semicolon, or pipe from the first non-blank line."),
        )
        .param(
            Param::enumv("date_format", ["iso", "us", "eu", "any"])
                .default("iso")
                .describe("Date style for columns declared as `date`: `iso` = YYYY-MM-DD (default), `us` = MM/DD/YYYY, `eu` = DD/MM/YYYY, `any` accepts all three."),
        )
        .param(
            Param::boolean("empty_ok")
                .default(true)
                .describe("Allow blank cells in checked columns. Default true; set false to require every checked cell to be non-empty and correctly typed."),
        )
        .param(
            Param::integer("max_issues")
                .default(50)
                .min(1.0)
                .max(1000.0)
                .describe("Maximum number of offending cells to list (1-1000, default 50). The summary still reports the full offending count."),
        )
        .param(
            Param::enumv("format", ["text", "json"])
                .default("text")
                .describe("Output format: `text` for a readable report (default) or `json` for a structured validation report."),
        )
}
fn schema_json() -> String { descriptor().to_schema_json() }

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/csv-column-type-validator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Validate CSV columns against declared types.",
    skill(
        description = "Validate CSV data against per-column declared types. Declare columns as `name:type` (or `1:type` with header=false), using int, float, bool, date, or enum(a|b|c). The tool checks every data cell in those columns and reports row, line, column, offending value, expected type, unknown declared columns, total cells checked, and truncation status. Supports delimiter auto-detection, header/headerless CSV, ISO/US/EU date formats, optional blank-cell requirements, text output, and JSON output.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "csv-column-type-validator", |a: Args| {
            gizza_ai_csv_column_type_validator_core::run(
                &a.data,
                &a.types,
                a.header,
                &a.delimiter,
                &a.date_format,
                a.empty_ok,
                a.max_issues as usize,
                &a.format,
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
    fn schema_json_exposes_declared_params() {
        let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let props = schema["properties"].as_object().unwrap();
        assert!(props.contains_key("data"));
        assert!(props.contains_key("types"));
        assert_eq!(props["delimiter"]["enum"][0], "auto");
        assert_eq!(props["date_format"]["enum"][3], "any");
        assert_eq!(props["max_issues"]["default"], 50);
        assert_eq!(schema["required"], serde_json::json!(["data", "types"]));
    }
}
