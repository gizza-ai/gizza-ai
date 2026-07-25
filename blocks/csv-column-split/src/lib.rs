//! gizza-ai/csv-column-split — split a CSV column into several columns or
//! concatenate several columns into one.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

fn default_mode() -> String {
    "split".into()
}
fn default_separator() -> String {
    ",".into()
}
fn default_max_columns() -> usize {
    0
}
fn default_true() -> bool {
    true
}
fn default_delimiter() -> String {
    ",".into()
}

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default = "default_mode")]
    mode: String,
    columns: String,
    #[serde(default = "default_separator")]
    separator: String,
    #[serde(default)]
    names: String,
    #[serde(default = "default_max_columns")]
    max_columns: usize,
    #[serde(default)]
    keep_source: bool,
    #[serde(default = "default_true")]
    trim: bool,
    #[serde(default = "default_true")]
    has_header: bool,
    #[serde(default = "default_delimiter")]
    delimiter: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().multiline().describe("Input CSV text to transform."))
        .param(Param::enumv("mode", ["split", "concat"]).default("split").describe("Operation to perform: split one source column into multiple columns (default), or concat multiple source columns into one."))
        .param(Param::string("columns").required().describe("Column or columns to use. In split mode, provide one header name or 1-based index. In concat mode, provide a comma-separated list of header names or 1-based indices in join order."))
        .param(Param::string("separator").default(",").describe("Literal separator string. Split mode splits cell values on this string; concat mode joins source values with it. Default comma."))
        .param(Param::string("names").describe("Output column names. Split mode: comma-separated names for the generated columns (missing names auto-fill as <source>_1, <source>_2). Concat mode: the combined column name."))
        .param(Param::integer("max_columns").min(0.0).max(100.0).default(0).describe("Split mode only: maximum number of output columns. 0 means split every occurrence; 2 means split on the first occurrence only. Default 0."))
        .param(Param::boolean("keep_source").default(false).describe("Keep the original source column(s) as well as the new column(s). Default false replaces the source column(s)."))
        .param(Param::boolean("trim").default(true).describe("Split mode only: trim whitespace around each split value. Default true."))
        .param(Param::boolean("has_header").default(true).describe("Treat the first CSV row as a header row so columns can be selected by name. Default true."))
        .param(Param::enumv("delimiter", [",", "tab", ";", "|"]).default(",").describe("CSV field delimiter for both input and output: comma (default), tab, semicolon, or pipe."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct CsvColumnSplit;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/csv-column-split",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Split a CSV column into several columns, or concatenate columns into one.",
    skill(
        description = "Transform CSV columns in either direction: split one source column into several generated columns by a literal separator (like Text to Columns), or concatenate multiple source columns into one with a chosen separator. Select columns by header name or 1-based index, optionally keep the source columns, trim split values, control maximum split columns, and choose comma/tab/semicolon/pipe CSV delimiters. Returns transformed CSV text.",
        parameters = schema_json()
    ),
)]
impl CsvColumnSplit {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "csv-column-split", |a: Args| {
            gizza_ai_csv_column_split_core::column_split(
                &a.data,
                &a.mode,
                &a.columns,
                &a.separator,
                &a.names,
                a.max_columns,
                a.keep_source,
                a.trim,
                a.has_header,
                &a.delimiter,
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
        let authored: serde_json::Value = serde_json::from_str(r#"{
          "type":"object","properties":{
            "data":{"type":"string","description":"Input CSV text to transform."},
            "mode":{"type":"string","enum":["split","concat"],"default":"split","description":"Operation to perform: split one source column into multiple columns (default), or concat multiple source columns into one."},
            "columns":{"type":"string","description":"Column or columns to use. In split mode, provide one header name or 1-based index. In concat mode, provide a comma-separated list of header names or 1-based indices in join order."},
            "separator":{"type":"string","default":",","description":"Literal separator string. Split mode splits cell values on this string; concat mode joins source values with it. Default comma."},
            "names":{"type":"string","description":"Output column names. Split mode: comma-separated names for the generated columns (missing names auto-fill as <source>_1, <source>_2). Concat mode: the combined column name."},
            "max_columns":{"type":"integer","minimum":0,"maximum":100,"default":0,"description":"Split mode only: maximum number of output columns. 0 means split every occurrence; 2 means split on the first occurrence only. Default 0."},
            "keep_source":{"type":"boolean","default":false,"description":"Keep the original source column(s) as well as the new column(s). Default false replaces the source column(s)."},
            "trim":{"type":"boolean","default":true,"description":"Split mode only: trim whitespace around each split value. Default true."},
            "has_header":{"type":"boolean","default":true,"description":"Treat the first CSV row as a header row so columns can be selected by name. Default true."},
            "delimiter":{"type":"string","enum":[",","tab",";","|"],"default":",","description":"CSV field delimiter for both input and output: comma (default), tab, semicolon, or pipe."}
          },"required":["data","columns"],"additionalProperties":false
        }"#).unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
