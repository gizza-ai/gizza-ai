//! gizza-ai/csv-ragged-row-padder — normalize ragged pasted CSV/TSV text.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default)]
    width: Option<i64>,
    #[serde(default = "default_width_from")]
    width_from: String,
    #[serde(default = "default_long_rows")]
    long_rows: String,
    #[serde(default)]
    pad_value: String,
    #[serde(default = "default_true")]
    header: bool,
    #[serde(default = "default_delimiter")]
    delimiter: String,
    #[serde(default = "default_true")]
    drop_empty_rows: bool,
    #[serde(default = "default_line_ending")]
    line_ending: String,
    #[serde(default = "default_output")]
    output: String,
}

fn default_width_from() -> String {
    "header".into()
}
fn default_long_rows() -> String {
    "truncate".into()
}
fn default_delimiter() -> String {
    "auto".into()
}
fn default_line_ending() -> String {
    "lf".into()
}
fn default_output() -> String {
    "csv".into()
}
fn default_true() -> bool {
    true
}

/// Single source for chat/CLI/page parameters.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("input").required().describe("CSV/TSV text to repair. Paste the complete table, including the header row if header=true."))
        .param(Param::integer("width").default(0).min(0.0).max(gizza_ai_csv_ragged_row_padder_core::MAX_WIDTH as f64).describe("Target number of fields per row. Use 0 (default) to infer the width from width_from."))
        .param(Param::enumv("width_from", ["header", "max", "mode"]).default("header").describe("How to infer target width when width=0: header uses the first row, max uses the widest row, mode uses the most common row width."))
        .param(Param::enumv("long_rows", ["truncate", "merge", "flag", "drop"]).default("truncate").describe("How to handle rows wider than the target: truncate extra fields, merge extras into the last column, flag them in a report while leaving them as-is, or drop them."))
        .param(Param::string("pad_value").default("").describe("Value appended to short data rows until they reach the target width. Empty string means blank CSV cells."))
        .param(Param::boolean("header").default(true).describe("Treat the first row as column names. Short headers are padded with generated column_N names instead of pad_value. Default true."))
        .param(Param::enumv("delimiter", ["auto", "comma", "tab", "semicolon", "pipe"]).default("auto").describe("Input/output delimiter. auto sniffs comma, semicolon, tab, or pipe outside quotes; choose one to force it."))
        .param(Param::boolean("drop_empty_rows").default(true).describe("Drop rows whose cells are all blank before measuring widths. Default true."))
        .param(Param::enumv("line_ending", ["lf", "crlf"]).default("lf").describe("Output line ending: lf (Unix, default) or crlf (Windows)."))
        .param(Param::enumv("output", ["csv", "report"]).default("csv").describe("Return the repaired CSV (default) or a plain-text report listing each padded, truncated, merged, flagged, or dropped row."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct CsvRaggedRowPadder;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/csv-ragged-row-padder",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Pad or trim ragged CSV rows to a uniform width",
    skill(
        description = "Repair ragged CSV/TSV text whose rows have inconsistent field counts. Short rows are padded with blanks or a chosen value; long rows can be truncated, merged into the last column, flagged in a report, or dropped. The tool can infer width from the header, widest row, or modal row width, sniff comma/tab/semicolon/pipe delimiters, preserve quoting, drop blank rows, generate missing header names, normalize line endings, and return either the repaired CSV or a diagnostic report.",
        parameters = schema_json()
    ),
)]
impl CsvRaggedRowPadder {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "csv-ragged-row-padder", |a: Args| {
            run_args(a).map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

fn run_args(a: Args) -> Result<String, String> {
    let width = a.width.unwrap_or(0);
    if width < 0 {
        return Err(format!(
            "width must be between 0 and {}, got {width}",
            gizza_ai_csv_ragged_row_padder_core::MAX_WIDTH
        ));
    }
    gizza_ai_csv_ragged_row_padder_core::pad_ragged(
        &a.input,
        width as usize,
        &a.width_from,
        &a.long_rows,
        &a.pad_value,
        a.header,
        &a.delimiter,
        a.drop_empty_rows,
        &a.line_ending,
        &a.output,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_has_expected_params() {
        let names: Vec<String> = descriptor().params.iter().map(|p| p.name.clone()).collect();
        assert_eq!(
            names,
            [
                "input",
                "width",
                "width_from",
                "long_rows",
                "pad_value",
                "header",
                "delimiter",
                "drop_empty_rows",
                "line_ending",
                "output"
            ]
        );
        let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(
            schema["properties"]["long_rows"]["enum"],
            serde_json::json!(["truncate", "merge", "flag", "drop"])
        );
        assert_eq!(schema["properties"]["delimiter"]["default"], "auto");
    }

    #[test]
    fn run_args_uses_defaults() {
        let out = run_args(Args {
            input: "a,b,c\n1,2\n".into(),
            width: None,
            width_from: default_width_from(),
            long_rows: default_long_rows(),
            pad_value: String::new(),
            header: true,
            delimiter: default_delimiter(),
            drop_empty_rows: true,
            line_ending: default_line_ending(),
            output: default_output(),
        })
        .unwrap();
        assert_eq!(out, "a,b,c\n1,2,\n");
    }
}
