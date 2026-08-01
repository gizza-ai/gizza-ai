//! gizza-ai/numeric-range-check — chat skill block on the shared tool abstraction.
//! Flags CSV numeric values that fall outside an expected min/max range.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

fn default_true() -> bool {
    true
}
fn default_max_issues() -> u64 {
    50
}

#[derive(Deserialize)]
struct Args {
    data: String,
    columns: String,
    #[serde(default)]
    min: Option<f64>,
    #[serde(default)]
    max: Option<f64>,
    #[serde(default = "default_true")]
    inclusive: bool,
    #[serde(default = "default_true")]
    header: bool,
    #[serde(default)]
    delimiter: String,
    #[serde(default)]
    non_numeric: String,
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
                .describe("CSV data to check. Paste rows as text; quoted fields and quoted newlines are supported. The first row is treated as a header by default."),
        )
        .param(
            Param::string("columns")
                .required()
                .describe("Which columns to range-check, comma- or newline-separated. Use header names (for example `age, price`) or 1-based indexes when header=false (for example `2, 4`). Use `all` to check every column. Non-numeric cells are handled per the non_numeric option."),
        )
        .param(
            Param::number("min")
                .describe("Lowest allowed value (inclusive by default). Optional if max is set — a one-sided check. Example: 0. Values below this are flagged."),
        )
        .param(
            Param::number("max")
                .describe("Highest allowed value (inclusive by default). Optional if min is set — a one-sided check. Example: 120. Values above this are flagged. Set at least one of min/max."),
        )
        .param(
            Param::boolean("inclusive")
                .default(true)
                .describe("Treat the bounds as inclusive so a value equal to min or max is in range. Default true. Set false for a strict/exclusive range where equal-to-a-bound values are flagged."),
        )
        .param(
            Param::boolean("header")
                .default(true)
                .describe("Treat the first row as a header row and resolve `columns` by name. Default true. Set false to reference columns by 1-based index."),
        )
        .param(
            Param::enumv("delimiter", ["auto", "comma", "tab", "semicolon", "pipe"])
                .default("auto")
                .describe("CSV delimiter. `auto` (default) detects comma, tab, semicolon, or pipe from the first non-blank line. Thousands separators like `1,000` are accepted only when the delimiter is not a comma."),
        )
        .param(
            Param::enumv("non_numeric", ["flag", "ignore"])
                .default("flag")
                .describe("How to treat non-empty cells that are not numbers: `flag` (default) reports them as violations; `ignore` skips them and range-checks only the numeric cells."),
        )
        .param(
            Param::boolean("empty_ok")
                .default(true)
                .describe("Allow blank cells in checked columns. Default true; set false to require every checked cell to be present (blank cells are then reported as `empty cell (required)`)."),
        )
        .param(
            Param::integer("max_issues")
                .default(50)
                .min(1.0)
                .max(1000.0)
                .describe("Maximum number of flagged cells to list (1-1000, default 50). The summary still reports the full flagged count."),
        )
        .param(
            Param::enumv("format", ["text", "json"])
                .default("text")
                .describe("Output format: `text` for a readable report (default) or `json` for a structured range-check report."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/numeric-range-check",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Flag CSV numeric values outside an expected min/max range.",
    skill(
        description = "Range-check CSV numeric columns against an expected min and/or max. Pick columns by header name, 1-based index, or `all`, set a min, a max, or both, and choose inclusive or exclusive bounds. The tool flags every data cell whose value falls outside the range and reports row, physical line, column, offending value, and reason, plus totals for cells checked, numeric cells, non-numeric cells, and the full flagged count with truncation status. Supports delimiter auto-detection, header/headerless CSV, one-sided ranges, flagging or ignoring non-numeric cells, optional required (non-blank) cells, text output, and JSON output.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "numeric-range-check", |a: Args| {
            gizza_ai_numeric_range_check_core::run(
                &a.data,
                &a.columns,
                a.min,
                a.max,
                a.inclusive,
                a.header,
                &a.delimiter,
                &a.non_numeric,
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
        assert!(props.contains_key("columns"));
        assert!(props.contains_key("min"));
        assert!(props.contains_key("max"));
        assert_eq!(props["delimiter"]["enum"][0], "auto");
        assert_eq!(props["non_numeric"]["enum"][0], "flag");
        assert_eq!(props["inclusive"]["default"], true);
        assert_eq!(props["max_issues"]["default"], 50);
        assert_eq!(schema["required"], serde_json::json!(["data", "columns"]));
    }
}
