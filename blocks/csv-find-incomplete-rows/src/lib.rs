//! gizza-ai/csv-find-incomplete-rows — flag malformed CSV rows (too few / too
//! many fields, or a blank cell in a required column). Thin wrapper; the chat
//! schema is single-sourced from descriptor() (which also drives the CLI);
//! handle() delegates to block_utils::run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_csv_find_incomplete_rows_core::find_incomplete;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default = "default_true")]
    header: bool,
    #[serde(default)]
    delimiter: String,
    #[serde(default)]
    required: String,
}
fn default_true() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe("The CSV text to scan."))
        .param(
            Param::boolean("header")
                .default(true)
                .describe("Treat the first row as a header naming the columns (default true)."),
        )
        .param(
            Param::string("delimiter").default(",").describe(
                "Field separator: a single char or 'comma'/'tab'/'semicolon'/'pipe'. Default ','.",
            ),
        )
        .param(Param::string("required").describe(
            "Optional comma-separated list of column names or 1-based indices that must be non-blank on every row (e.g. \"email,phone\" or \"1,3\"). Blank cells in these columns are flagged.",
        ))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/csv-find-incomplete-rows",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Flag CSV rows with the wrong field count or blank required cells",
    skill(
        description = "Scan a CSV and flag incomplete or malformed rows: rows with too few or too many fields (relative to the header/first-row width), and rows with a blank cell in a column you mark as required. Returns the expected field count, the column names, and every flagged row with its line number, field count, issue types, and blank required columns. delimiter is a single char or comma/tab/semicolon/pipe; required is a comma-separated list of column names or 1-based indices. Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "csv-find-incomplete-rows", |a: Args| {
            let delim = if a.delimiter.is_empty() { ",".to_string() } else { a.delimiter };
            find_incomplete(&a.data, a.header, &delim, &a.required).map_err(SkillError::InvalidArgs)
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
                    "data": { "type": "string", "description": "The CSV text to scan." },
                    "header": { "type": "boolean", "default": true, "description": "Treat the first row as a header naming the columns (default true)." },
                    "delimiter": { "type": "string", "default": ",", "description": "Field separator: a single char or 'comma'/'tab'/'semicolon'/'pipe'. Default ','." },
                    "required": { "type": "string", "description": "Optional comma-separated list of column names or 1-based indices that must be non-blank on every row (e.g. \"email,phone\" or \"1,3\"). Blank cells in these columns are flagged." }
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
