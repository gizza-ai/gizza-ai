//! gizza-ai/csv-pivot — build a pivot table from a CSV.
//! Thin wrapper around the core; chat schema single-sourced from descriptor();
//! handler delegates to run_skill. Pure.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_csv_pivot_core::pivot;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    rows: String,
    columns: String,
    values: String,
    #[serde(default = "default_sum")]
    agg: String,
    #[serde(default)]
    delimiter: String,
}
fn default_sum() -> String { "sum".to_string() }

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe("The CSV text (first row must be a header)."))
        .param(Param::string("rows").required().describe("Row-key column(s) — comma-separated names or 1-based indices. Their distinct combinations become output rows."))
        .param(Param::string("columns").required().describe("The column whose distinct values become the pivot's output columns."))
        .param(Param::string("values").required().describe("The column to aggregate into each cell."))
        .param(Param::enumv("agg", ["sum", "count", "avg", "min", "max"]).default("sum").describe("Cell aggregation (default sum)."))
        .param(Param::string("delimiter").default(",").describe("Field separator: a single char or 'comma'/'tab'/'semicolon'/'pipe'. Default ','."))
}

fn schema_json() -> String { descriptor().to_schema_json() }

#[cfg(target_arch = "wasm32")]
struct CsvPivot;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/csv-pivot",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Build a pivot table from a CSV",
    skill(
        description = "Build a pivot table: group rows by one or more `rows` columns, spread the distinct values of the `columns` field into output columns, and fill each cell by aggregating the `values` column (agg = sum/count/avg/min/max). Row keys and pivot columns are in first-seen order; empty cells are blank. Requires a header row.",
        parameters = schema_json()
    )
)]
impl CsvPivot {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "csv-pivot", |a: Args| {
            let delim = if a.delimiter.is_empty() { ",".to_string() } else { a.delimiter };
            pivot(&a.data, &a.rows, &a.columns, &a.values, &a.agg, true, &delim).map_err(SkillError::InvalidArgs)
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
                    "data":      { "type": "string", "description": "The CSV text (first row must be a header)." },
                    "rows":      { "type": "string", "description": "Row-key column(s) — comma-separated names or 1-based indices. Their distinct combinations become output rows." },
                    "columns":   { "type": "string", "description": "The column whose distinct values become the pivot's output columns." },
                    "values":    { "type": "string", "description": "The column to aggregate into each cell." },
                    "agg":       { "type": "string", "enum": ["sum", "count", "avg", "min", "max"], "default": "sum", "description": "Cell aggregation (default sum)." },
                    "delimiter": { "type": "string", "default": ",", "description": "Field separator: a single char or 'comma'/'tab'/'semicolon'/'pipe'. Default ','." }
                },
                "required": ["data", "rows", "columns", "values"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
