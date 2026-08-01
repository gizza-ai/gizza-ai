//! gizza-ai/csv-fill-down — forward-fill empty CSV cells with the last non-empty
//! value above them (spreadsheet "fill down"), or back-fill from below with
//! direction=up. Thin wrapper around the core; chat schema single-sourced from
//! descriptor(); handler delegates to run_skill. Pure.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_csv_fill_down_core::fill_down;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default)]
    columns: String,
    #[serde(default)]
    direction: String,
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
        .param(Param::string("data").required().describe("The CSV text to fill. Empty (or whitespace-only) cells are filled from the nearest non-empty value in their column."))
        .param(Param::string("columns").default("").describe("Optional comma-separated columns to fill (1-based indices, or header names when header=true), e.g. 'region' or '1,3'. Empty = fill every column."))
        .param(Param::enumv("direction", ["down", "up"]).default("down").describe("Fill direction: 'down' carries the last non-empty value above into blanks below it; 'up' carries the next non-empty value below into blanks above it. Default 'down'."))
        .param(Param::boolean("header").default(true).describe("Treat the first row as a header: keep it verbatim (never filled) and allow naming columns. Default true."))
        .param(Param::string("delimiter").default(",").describe("Field separator: a single char or 'comma'/'tab'/'semicolon'/'pipe'. Default ','."))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct CsvFillDown;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/csv-fill-down",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Forward-fill empty CSV cells with the last non-empty value above",
    skill(
        description = "Forward-fill empty cells in a CSV with the last non-empty value above them, like a spreadsheet fill-down. A cell is empty when it is blank or whitespace-only. Set `direction`=up to back-fill from the next value below instead. `columns` (1-based indices or header names) limits filling to chosen columns; empty fills every column. `header`=true keeps the first row and lets you name columns. `delimiter` is a char or comma/tab/semicolon/pipe. Empties with no value to carry (leading cells on fill-down, trailing on fill-up) stay empty.",
        parameters = schema_json()
    ),
)]
impl CsvFillDown {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "csv-fill-down", |a: Args| {
            let delim = if a.delimiter.is_empty() {
                ",".to_string()
            } else {
                a.delimiter
            };
            let dir = if a.direction.is_empty() {
                "down".to_string()
            } else {
                a.direction
            };
            fill_down(&a.data, &a.columns, &dir, a.header, &delim).map_err(SkillError::InvalidArgs)
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
                    "data":      { "type": "string", "description": "The CSV text to fill. Empty (or whitespace-only) cells are filled from the nearest non-empty value in their column." },
                    "columns":   { "type": "string", "default": "", "description": "Optional comma-separated columns to fill (1-based indices, or header names when header=true), e.g. 'region' or '1,3'. Empty = fill every column." },
                    "direction": { "type": "string", "enum": ["down", "up"], "default": "down", "description": "Fill direction: 'down' carries the last non-empty value above into blanks below it; 'up' carries the next non-empty value below into blanks above it. Default 'down'." },
                    "header":    { "type": "boolean", "default": true, "description": "Treat the first row as a header: keep it verbatim (never filled) and allow naming columns. Default true." },
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
