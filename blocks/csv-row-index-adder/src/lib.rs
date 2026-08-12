//! gizza-ai/csv-row-index-adder — add a generated key column to CSV text.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use std::time::{SystemTime, UNIX_EPOCH};
use wafer_sdk::*;

fn default_mode() -> String {
    "sequential".into()
}
fn default_position() -> String {
    "start".into()
}
fn default_has_header() -> bool {
    true
}
fn default_start() -> i64 {
    1
}
fn default_step() -> i64 {
    1
}
fn default_separator() -> String {
    "-".into()
}
fn default_uuid_version() -> String {
    "4".into()
}
fn default_uuid_format() -> String {
    "standard".into()
}
fn default_delimiter() -> String {
    "auto".into()
}

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default = "default_mode")]
    mode: String,
    #[serde(default)]
    column_name: String,
    #[serde(default = "default_position")]
    position: String,
    #[serde(default)]
    reference_column: String,
    #[serde(default = "default_has_header")]
    has_header: bool,
    #[serde(default = "default_start")]
    start: i64,
    #[serde(default = "default_step")]
    step: i64,
    #[serde(default)]
    pad_width: i64,
    #[serde(default)]
    prefix: String,
    #[serde(default)]
    suffix: String,
    #[serde(default)]
    columns: String,
    #[serde(default = "default_separator")]
    separator: String,
    #[serde(default = "default_uuid_version")]
    uuid_version: String,
    #[serde(default = "default_uuid_format")]
    uuid_format: String,
    #[serde(default = "default_delimiter")]
    delimiter: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe("CSV or delimited text to update. Paste the full table; the output preserves CSV quoting and the chosen delimiter."))
        .param(Param::enumv("mode", ["sequential", "uuid", "composite"]).default("sequential").describe("Generated value type: sequential numbers, UUIDs, or a composite key joined from existing columns."))
        .param(Param::string("column_name").default("").describe("Header for the new column. Leave blank to use index, uuid, or key based on mode."))
        .param(Param::enumv("position", ["start", "end", "before", "after"]).default("start").describe("Where to insert the generated column: at the start, at the end, before a reference column, or after a reference column."))
        .param(Param::string("reference_column").default("").describe("Header name or 1-based column number used when position is before or after."))
        .param(Param::boolean("has_header").default(true).describe("Treat the first row as a header row. When true, the first row receives column_name instead of a generated value."))
        .param(Param::integer("start").default(1).describe("First sequential number. Use 0 for zero-based indexing. Ignored unless mode is sequential."))
        .param(Param::integer("step").default(1).describe("Increment between sequential numbers. Negative steps count down; zero is rejected. Ignored unless mode is sequential."))
        .param(Param::integer("pad_width").min(0.0).max(64.0).default(0).describe("Zero-pad sequential numbers to this many digits (0-64). Prefix and suffix are added after padding."))
        .param(Param::string("prefix").default("").describe("Text prepended to each generated value, e.g. INV-."))
        .param(Param::string("suffix").default("").describe("Text appended to each generated value, e.g. -2026."))
        .param(Param::string("columns").default("").describe("Composite mode only: comma-separated source columns, by header name or 1-based number, to join into the key."))
        .param(Param::string("separator").default("-").describe("Composite mode separator between source column values. Default '-' ."))
        .param(Param::enumv("uuid_version", ["4", "7"]).default("4").describe("UUID mode only: v4 random UUIDs or v7 time-ordered UUIDs."))
        .param(Param::enumv("uuid_format", ["standard", "uppercase", "compact", "braces", "urn"]).default("standard").describe("UUID rendering: standard lowercase with hyphens, uppercase, compact no-hyphens, braces, or urn."))
        .param(Param::enumv("delimiter", ["auto", ",", "tab", ";", "|"]).default("auto").describe("Input delimiter. auto sniffs comma, tab, semicolon, or pipe from the first non-empty line; output uses the same delimiter."))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn run_args(a: Args) -> Result<String, SkillError> {
    gizza_ai_csv_row_index_adder_core::add_index(
        &a.data,
        &a.mode,
        &a.column_name,
        &a.position,
        &a.reference_column,
        a.has_header,
        a.start,
        a.step,
        a.pad_width,
        &a.prefix,
        &a.suffix,
        &a.columns,
        &a.separator,
        &a.uuid_version,
        &a.uuid_format,
        &a.delimiter,
        now_ms(),
    )
    .map_err(SkillError::InvalidArgs)
}

#[cfg(target_arch = "wasm32")]
struct CsvRowIndexAdder;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/csv-row-index-adder",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Add sequential, UUID, or composite key columns to CSV rows",
    skill(
        description = "Add a generated key column to CSV text. mode=sequential inserts row numbers with start, step, zero-padding, prefix and suffix. mode=uuid adds one v4 random or v7 time-ordered UUID per data row. mode=composite builds a key by joining existing columns. Choose whether the input has a header, where the new column is inserted, and whether the delimiter is auto-detected or fixed to comma, tab, semicolon, or pipe. Returns updated CSV text.",
        parameters = schema_json()
    ),
)]
impl CsvRowIndexAdder {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "csv-row-index-adder", run_args) {
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
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived["type"], "object");
        assert_eq!(derived["required"], serde_json::json!(["data"]));
        assert_eq!(
            derived["properties"]["mode"]["enum"],
            serde_json::json!(["sequential", "uuid", "composite"])
        );
        assert_eq!(derived["properties"]["has_header"]["default"], true);
        assert_eq!(derived["properties"]["pad_width"]["maximum"], 64);
    }

    #[test]
    fn run_args_happy_path() {
        let a: Args =
            serde_json::from_str(r#"{"data":"name,city\nAda,London\nLin,Taipei"}"#).unwrap();
        assert_eq!(
            run_args(a).unwrap(),
            "index,name,city\n1,Ada,London\n2,Lin,Taipei\n"
        );
    }

    #[test]
    fn run_args_composite_key() {
        let a: Args = serde_json::from_str(r#"{"data":"region,dept\nEU,ops","mode":"composite","columns":"region,dept","column_name":"key"}"#).unwrap();
        assert_eq!(run_args(a).unwrap(), "key,region,dept\nEU-ops,EU,ops\n");
    }

    #[test]
    fn run_args_rejects_bad_mode() {
        let a: Args = serde_json::from_str(r#"{"data":"a\nx","mode":"bad"}"#).unwrap();
        assert!(run_args(a).is_err());
    }
}
