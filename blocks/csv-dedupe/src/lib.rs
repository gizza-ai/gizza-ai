//! gizza-ai/csv-dedupe — remove duplicate CSV rows (first kept).
//! Thin wrapper around the core; chat schema single-sourced from descriptor();
//! handler delegates to run_skill. Pure.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_csv_dedupe_core::dedupe;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default)]
    columns: String,
    #[serde(default = "default_true")]
    header: bool,
    #[serde(default)]
    delimiter: String,
}
fn default_true() -> bool { true }

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe("The CSV text to de-duplicate."))
        .param(Param::string("columns").default("").describe("Optional comma-separated key columns (1-based indices, or header names when header=true). Empty = match the whole row."))
        .param(Param::boolean("header").default(true).describe("Treat the first row as a header (kept, and matchable by name). Default true."))
        .param(Param::string("delimiter").default(",").describe("Field separator: a single char or 'comma'/'tab'/'semicolon'/'pipe'. Default ','."))
}

fn schema_json() -> String { descriptor().to_schema_json() }

#[cfg(target_arch = "wasm32")]
struct CsvDedupe;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/csv-dedupe",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Remove duplicate rows from a CSV",
    skill(
        description = "Remove duplicate rows from CSV, keeping the first occurrence. By default a row is a duplicate when the whole row matches; set `columns` (1-based indices or header names) to key the dedup on a subset of columns. `header`=true keeps the first row and lets you name columns. `delimiter` is a char or comma/tab/semicolon/pipe.",
        parameters = schema_json()
    )
)]
impl CsvDedupe {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "csv-dedupe", |a: Args| {
            let delim = if a.delimiter.is_empty() { ",".to_string() } else { a.delimiter };
            dedupe(&a.data, &a.columns, a.header, &delim).map_err(SkillError::InvalidArgs)
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
                    "data":      { "type": "string", "description": "The CSV text to de-duplicate." },
                    "columns":   { "type": "string", "default": "", "description": "Optional comma-separated key columns (1-based indices, or header names when header=true). Empty = match the whole row." },
                    "header":    { "type": "boolean", "default": true, "description": "Treat the first row as a header (kept, and matchable by name). Default true." },
                    "delimiter": { "type": "string", "default": ",", "description": "Field separator: a single char or 'comma'/'tab'/'semicolon'/'pipe'. Default ','." }
                },
                "required": ["data"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
