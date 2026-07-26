//! gizza-ai/csv-collapse-rows — group CSV rows and collapse a column into one cell.
//! Thin wrapper around the core; chat schema single-sourced from descriptor();
//! handler delegates to run_skill. Pure.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_csv_collapse_rows_core::collapse_rows;
use serde::Deserialize;
use wafer_sdk::*;

fn yes() -> bool {
    true
}

#[derive(Deserialize)]
struct Args {
    data: String,
    key_columns: String,
    collapse_column: String,
    #[serde(default)]
    separator: String,
    #[serde(default)]
    dedupe: bool,
    #[serde(default = "yes")]
    skip_empty: bool,
    #[serde(default)]
    sort_values: String,
    #[serde(default)]
    delimiter: String,
    #[serde(default = "yes")]
    has_header: bool,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe("The CSV text (first row must be a header)."))
        .param(Param::string("key_columns").required().describe("Column(s) to group by — comma-separated header names or 1-based indices."))
        .param(Param::string("collapse_column").required().describe("The column whose values are collapsed into one delimited cell (header name or 1-based index)."))
        .param(Param::string("separator").default(", ").describe("String inserted between collapsed values. Default ', '."))
        .param(Param::boolean("dedupe").default(false).describe("Drop duplicate values within each group (like SQL DISTINCT)."))
        .param(Param::boolean("skip_empty").default(true).describe("Ignore blank cells in the collapse column."))
        .param(Param::enumv("sort_values", ["none", "asc", "desc"]).default("none").describe("Order collapsed values within each cell: none (first-seen), asc, or desc."))
        .param(Param::enumv("delimiter", ["comma", "tab", "semicolon", "pipe"]).default("comma").describe("CSV field delimiter for input and output."))
        .param(Param::boolean("has_header").default(true).describe("First row is a header. When on, columns may be referenced by name or 1-based index; when off, use 1-based indices and output columns are labelled col_1, col_2, …"))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct CsvCollapseRows;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/csv-collapse-rows",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Group CSV rows and collapse a column into one delimited cell",
    skill(
        description = "Group CSV rows by one or more key columns and collapse a chosen column's values from every row in the group into a single delimited cell — the CSV equivalent of SQL GROUP_CONCAT. `key_columns` is a comma-separated list of header names/indices; `collapse_column` is the column to collapse. Optionally dedupe (DISTINCT), sort the values, skip blanks, and pick the field delimiter. Output has one row per group (first-seen order): the key columns then the collapsed column. Requires a header row.",
        parameters = schema_json()
    ),
)]
impl CsvCollapseRows {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "csv-collapse-rows", |a: Args| {
            let delim = if a.delimiter.is_empty() { "comma".to_string() } else { a.delimiter };
            let sort = if a.sort_values.is_empty() { "none".to_string() } else { a.sort_values };
            collapse_rows(
                &a.data,
                &a.key_columns,
                &a.collapse_column,
                &a.separator,
                a.dedupe,
                a.skip_empty,
                &sort,
                &delim,
                a.has_header,
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
                    "data":            { "type": "string", "description": "The CSV text (first row must be a header)." },
                    "key_columns":     { "type": "string", "description": "Column(s) to group by — comma-separated header names or 1-based indices." },
                    "collapse_column": { "type": "string", "description": "The column whose values are collapsed into one delimited cell (header name or 1-based index)." },
                    "separator":       { "type": "string", "default": ", ", "description": "String inserted between collapsed values. Default ', '." },
                    "dedupe":          { "type": "boolean", "default": false, "description": "Drop duplicate values within each group (like SQL DISTINCT)." },
                    "skip_empty":      { "type": "boolean", "default": true, "description": "Ignore blank cells in the collapse column." },
                    "sort_values":     { "type": "string", "enum": ["none", "asc", "desc"], "default": "none", "description": "Order collapsed values within each cell: none (first-seen), asc, or desc." },
                    "delimiter":       { "type": "string", "enum": ["comma", "tab", "semicolon", "pipe"], "default": "comma", "description": "CSV field delimiter for input and output." },
                    "has_header":      { "type": "boolean", "default": true, "description": "First row is a header. When on, columns may be referenced by name or 1-based index; when off, use 1-based indices and output columns are labelled col_1, col_2, …" }
                },
                "required": ["data", "key_columns", "collapse_column"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
