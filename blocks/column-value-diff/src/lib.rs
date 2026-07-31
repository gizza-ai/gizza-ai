//! gizza-ai/column-value-diff — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    left: String,
    right: String,
    key: String,
    value: String,
    #[serde(default = "default_delimiter")]
    delimiter: String,
    #[serde(default = "default_true")]
    header: bool,
    #[serde(default)]
    ignore_case: bool,
    #[serde(default)]
    ignore_whitespace: bool,
    #[serde(default)]
    include_unmatched: bool,
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

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("left").required().describe("The original/left CSV text (the \"old\" table)."))
        .param(Param::string("right").required().describe("The updated/right CSV text (the \"new\" table)."))
        .param(Param::string("key").required().describe("Column(s) to join the two tables on — the identity of each row. Comma-separated for a composite key. Reference by header name (or 1-based index when header=false). Example: id or first,last."))
        .param(Param::string("value").required().describe("The single column whose values to compare between the matched rows — the metric you care about. Header name, or 1-based index when header=false. Example: price."))
        .param(Param::enumv("delimiter", ["comma", "tab", "semicolon", "pipe"]).default("comma").describe("Field delimiter used by both CSVs."))
        .param(Param::boolean("header").default(true).describe("Treat the first row of each CSV as a header, so key/value can be named columns. Turn off to reference columns by 1-based index (col1, col2, …)."))
        .param(Param::boolean("ignore_case").default(false).describe("Compare key and value text case-insensitively while preserving original text in the output."))
        .param(Param::boolean("ignore_whitespace").default(false).describe("Normalize runs of whitespace when comparing (so \"in  stock\" == \"in stock\") while preserving original text."))
        .param(Param::boolean("include_unmatched").default(false).describe("Also report keys present on only one side (left-only / right-only). Off by default so the report shows only value changes for keys in both tables."))
        .param(Param::enumv("format", ["table", "json", "csv"]).default("table").describe("Output shape: table (readable key → old/new report), json (structured), or csv (flat key,status,old,new change-log)."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/column-value-diff",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Join two tables on a key column and report where a chosen value column disagrees, with old/new pairs",
    skill(
        description = "Join two CSV tables on one or more key columns and compare a single chosen value column, reporting every key whose value disagrees as a clean old → new pair. Unlike a full cell-by-cell diff, every other column is ignored, so two files with different surrounding schemas still reconcile cleanly on the one metric you care about (price, quantity, status, …). Rows are matched by the composite key (referenced by header name or 1-based index); duplicate keys pair in order. Optionally report keys present on only one side (left-only / right-only). Optional case- and whitespace-insensitive matching. Output as a readable table, structured JSON, or a flat CSV change-log.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "column-value-diff", |a: Args| {
            gizza_ai_column_value_diff_core::run(
                &a.left,
                &a.right,
                &a.key,
                &a.value,
                &a.delimiter,
                a.header,
                a.ignore_case,
                a.ignore_whitespace,
                a.include_unmatched,
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
                "left": { "type": "string", "description": "The original/left CSV text (the \"old\" table)." },
                "right": { "type": "string", "description": "The updated/right CSV text (the \"new\" table)." },
                "key": { "type": "string", "description": "Column(s) to join the two tables on — the identity of each row. Comma-separated for a composite key. Reference by header name (or 1-based index when header=false). Example: id or first,last." },
                "value": { "type": "string", "description": "The single column whose values to compare between the matched rows — the metric you care about. Header name, or 1-based index when header=false. Example: price." },
                "delimiter": { "type": "string", "enum": ["comma", "tab", "semicolon", "pipe"], "default": "comma", "description": "Field delimiter used by both CSVs." },
                "header": { "type": "boolean", "default": true, "description": "Treat the first row of each CSV as a header, so key/value can be named columns. Turn off to reference columns by 1-based index (col1, col2, …)." },
                "ignore_case": { "type": "boolean", "default": false, "description": "Compare key and value text case-insensitively while preserving original text in the output." },
                "ignore_whitespace": { "type": "boolean", "default": false, "description": "Normalize runs of whitespace when comparing (so \"in  stock\" == \"in stock\") while preserving original text." },
                "include_unmatched": { "type": "boolean", "default": false, "description": "Also report keys present on only one side (left-only / right-only). Off by default so the report shows only value changes for keys in both tables." },
                "format": { "type": "string", "enum": ["table", "json", "csv"], "default": "table", "description": "Output shape: table (readable key → old/new report), json (structured), or csv (flat key,status,old,new change-log)." }
            },
            "required": ["left", "right", "key", "value"],
            "additionalProperties": false
        }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
