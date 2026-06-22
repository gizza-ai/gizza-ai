//! gizza-ai/csv-group-by — group CSV rows and aggregate columns.
//! Thin wrapper around the core; chat schema single-sourced from descriptor();
//! handler delegates to run_skill. Pure.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_csv_group_by_core::group_by;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    group_by: String,
    aggregations: String,
    #[serde(default)]
    delimiter: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe("The CSV text (first row must be a header)."))
        .param(Param::string("group_by").required().describe("Column(s) to group by — comma-separated header names or 1-based indices."))
        .param(Param::string("aggregations").required().describe("Aggregations as a comma/semicolon list of 'column:func' (func = count/sum/avg/min/max), or bare 'count' for the per-group row count. E.g. 'amount:sum, qty:avg, count'."))
        .param(Param::string("delimiter").default(",").describe("Field separator: a single char or 'comma'/'tab'/'semicolon'/'pipe'. Default ','."))
}

fn schema_json() -> String { descriptor().to_schema_json() }

#[cfg(target_arch = "wasm32")]
struct CsvGroupBy;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/csv-group-by",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Group CSV rows and aggregate columns",
    skill(
        description = "Group CSV rows by one or more columns and aggregate other columns. `group_by` is comma-separated column names/indices; `aggregations` is a comma/semicolon list of 'column:func' (func = count/sum/avg/min/max) plus optional bare 'count' for the row count. Output has one row per group (first-seen order) with the group columns then the aggregated columns (named like 'sum_amount'). Requires a header row.",
        parameters = schema_json()
    )
)]
impl CsvGroupBy {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "csv-group-by", |a: Args| {
            let delim = if a.delimiter.is_empty() { ",".to_string() } else { a.delimiter };
            group_by(&a.data, &a.group_by, &a.aggregations, true, &delim).map_err(SkillError::InvalidArgs)
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
                    "data":         { "type": "string", "description": "The CSV text (first row must be a header)." },
                    "group_by":     { "type": "string", "description": "Column(s) to group by — comma-separated header names or 1-based indices." },
                    "aggregations": { "type": "string", "description": "Aggregations as a comma/semicolon list of 'column:func' (func = count/sum/avg/min/max), or bare 'count' for the per-group row count. E.g. 'amount:sum, qty:avg, count'." },
                    "delimiter":    { "type": "string", "default": ",", "description": "Field separator: a single char or 'comma'/'tab'/'semicolon'/'pipe'. Default ','." }
                },
                "required": ["data", "group_by", "aggregations"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
