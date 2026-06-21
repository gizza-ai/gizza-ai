//! gizza-ai/csv-reorder-columns — reorder, swap, or drop CSV columns to a target
//! order. Thin wrapper; chat schema single-sourced from descriptor(); handler
//! delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_csv_reorder_columns_core::reorder;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    columns: String,
    #[serde(default = "default_true")]
    header: bool,
    #[serde(default)]
    delimiter: String,
}
fn default_true() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe("The CSV text."))
        .param(Param::string("columns").required().describe(
            "Target column order, comma-separated: column names (when header=true) or 1-based indices. Columns you omit are dropped; repeat a column to duplicate it. e.g. 'name,city' or '3,1'.",
        ))
        .param(
            Param::boolean("header")
                .default(true)
                .describe("Treat the first row as a header so columns can be named (default true). With false, use 1-based indices."),
        )
        .param(
            Param::string("delimiter")
                .default(",")
                .describe("Field separator: a single char or 'comma'/'tab'/'semicolon'/'pipe'. Default ','."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct CsvReorderColumns;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/csv-reorder-columns",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Reorder, swap, or drop CSV columns",
    skill(
        description = "Reorder, swap, or drop the columns of a CSV to match a target order. `columns` is a comma-separated list of column names (when header=true) or 1-based indices; columns are output in that order, omitted columns are dropped, and a repeated column is duplicated. delimiter is a single char or comma/tab/semicolon/pipe. Runs locally.",
        parameters = schema_json()
    ),
)]
impl CsvReorderColumns {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "csv-reorder-columns", |a: Args| {
            let delim = if a.delimiter.is_empty() { ",".to_string() } else { a.delimiter };
            reorder(&a.data, &a.columns, a.header, &delim).map_err(SkillError::InvalidArgs)
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
                    "data": { "type": "string", "description": "The CSV text." },
                    "columns": { "type": "string", "description": "Target column order, comma-separated: column names (when header=true) or 1-based indices. Columns you omit are dropped; repeat a column to duplicate it. e.g. 'name,city' or '3,1'." },
                    "header": { "type": "boolean", "default": true, "description": "Treat the first row as a header so columns can be named (default true). With false, use 1-based indices." },
                    "delimiter": { "type": "string", "default": ",", "description": "Field separator: a single char or 'comma'/'tab'/'semicolon'/'pipe'. Default ','." }
                },
                "required": ["data", "columns"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
