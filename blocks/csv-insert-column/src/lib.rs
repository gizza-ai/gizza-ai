//! gizza-ai/csv-insert-column — insert a constant-filled column into a CSV.
//! Thin wrapper around the core; chat schema single-sourced from descriptor();
//! handler delegates to run_skill. Pure.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_csv_insert_column_core::insert_column;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    name: String,
    #[serde(default)]
    value: String,
    #[serde(default = "default_end")]
    position: String,
    #[serde(default = "default_true")]
    header: bool,
    #[serde(default)]
    delimiter: String,
}
fn default_end() -> String { "end".to_string() }
fn default_true() -> bool { true }

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe("The CSV text."))
        .param(Param::string("name").required().describe("Header name for the new column (used when header=true)."))
        .param(Param::string("value").default("").describe("Constant value to fill the new column in every data row. Default empty."))
        .param(Param::string("position").default("end").describe("1-based position to insert at, or 'end' to append (default). Clamped to the row width."))
        .param(Param::boolean("header").default(true).describe("Treat the first row as a header (gets `name`; data rows get `value`). Default true."))
        .param(Param::string("delimiter").default(",").describe("Field separator: a single char or 'comma'/'tab'/'semicolon'/'pipe'. Default ','."))
}

fn schema_json() -> String { descriptor().to_schema_json() }

#[cfg(target_arch = "wasm32")]
struct CsvInsertColumn;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/csv-insert-column",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Insert a constant column into a CSV",
    skill(
        description = "Insert a new column into a CSV at a chosen 1-based position (or 'end' to append), filling every data row with a constant `value`. The header row gets the column `name`. Useful for adding an id/source/constant flag column. `position` is clamped to the row width.",
        parameters = schema_json()
    )
)]
impl CsvInsertColumn {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "csv-insert-column", |a: Args| {
            let delim = if a.delimiter.is_empty() { ",".to_string() } else { a.delimiter };
            insert_column(&a.data, &a.name, &a.value, &a.position, a.header, &delim).map_err(SkillError::InvalidArgs)
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
                    "data":      { "type": "string", "description": "The CSV text." },
                    "name":      { "type": "string", "description": "Header name for the new column (used when header=true)." },
                    "value":     { "type": "string", "default": "", "description": "Constant value to fill the new column in every data row. Default empty." },
                    "position":  { "type": "string", "default": "end", "description": "1-based position to insert at, or 'end' to append (default). Clamped to the row width." },
                    "header":    { "type": "boolean", "default": true, "description": "Treat the first row as a header (gets `name`; data rows get `value`). Default true." },
                    "delimiter": { "type": "string", "default": ",", "description": "Field separator: a single char or 'comma'/'tab'/'semicolon'/'pipe'. Default ','." }
                },
                "required": ["data", "name"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
