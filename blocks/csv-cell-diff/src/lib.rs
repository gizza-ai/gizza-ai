//! gizza-ai/csv-cell-diff — column-aligned, cell-level diff of two CSVs.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    left: String,
    right: String,
    #[serde(default)]
    key: String,
    #[serde(default = "default_delimiter")]
    delimiter: String,
    #[serde(default = "default_true")]
    header: bool,
    #[serde(default)]
    ignore_case: bool,
    #[serde(default)]
    ignore_whitespace: bool,
    #[serde(default = "default_format")]
    format: String,
}

fn default_delimiter() -> String {
    "comma".to_string()
}
fn default_true() -> bool {
    true
}
fn default_format() -> String {
    "table".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("left").required().describe("The original/left CSV text."))
        .param(Param::string("right").required().describe("The updated/right CSV text."))
        .param(Param::string("key").default("").describe("Comma-separated key column name(s) (or 1-based index/indices) to pair rows by, so reordered rows still match. Leave empty to pair rows positionally by order. Example: id or first,last."))
        .param(Param::enumv("delimiter", ["comma", "tab", "semicolon", "pipe"]).default("comma").describe("Field delimiter used by both CSVs."))
        .param(Param::boolean("header").default(true).describe("Treat the first row as a header and align columns by name (reordered columns still match). Turn off to align columns by position (col1, col2, …)."))
        .param(Param::boolean("ignore_case").default(false).describe("Compare cell and key values case-insensitively while preserving original text in the output."))
        .param(Param::boolean("ignore_whitespace").default(false).describe("Normalize runs of whitespace when comparing while preserving original text in the output."))
        .param(Param::enumv("format", ["table", "json", "csv"]).default("table").describe("Output: table (readable report), json (structured report), or csv (flat change-log)."))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/csv-cell-diff",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Compare two CSVs column-by-column and highlight every differing cell",
    skill(
        description = "Align two CSVs column-by-column (by header name, so reordered columns still match) and compare them cell-by-cell, reporting every individual cell that changed with its old and new value, plus which rows and whole columns were added or removed. Pair rows by one or more key columns so reordered rows still match, or positionally when no key is given. Optional case- and whitespace-insensitive matching. Output as a readable table report, a structured JSON report, or a flat CSV change-log.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "csv-cell-diff", |a: Args| {
            gizza_ai_csv_cell_diff_core::run(
                &a.left,
                &a.right,
                &a.key,
                &a.delimiter,
                a.header,
                a.ignore_case,
                a.ignore_whitespace,
                &a.format,
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
                "left": { "type": "string", "description": "The original/left CSV text." },
                "right": { "type": "string", "description": "The updated/right CSV text." },
                "key": { "type": "string", "default": "", "description": "Comma-separated key column name(s) (or 1-based index/indices) to pair rows by, so reordered rows still match. Leave empty to pair rows positionally by order. Example: id or first,last." },
                "delimiter": { "type": "string", "enum": ["comma", "tab", "semicolon", "pipe"], "default": "comma", "description": "Field delimiter used by both CSVs." },
                "header": { "type": "boolean", "default": true, "description": "Treat the first row as a header and align columns by name (reordered columns still match). Turn off to align columns by position (col1, col2, …)." },
                "ignore_case": { "type": "boolean", "default": false, "description": "Compare cell and key values case-insensitively while preserving original text in the output." },
                "ignore_whitespace": { "type": "boolean", "default": false, "description": "Normalize runs of whitespace when comparing while preserving original text in the output." },
                "format": { "type": "string", "enum": ["table", "json", "csv"], "default": "table", "description": "Output: table (readable report), json (structured report), or csv (flat change-log)." }
            },
            "required": ["left", "right"],
            "additionalProperties": false
        }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
