//! gizza-ai/sql-dump-to-csv — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI + page); handle() delegates to block_utils::run_skill. No host calls —
//! runs entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    sql: String,
    #[serde(default)]
    table: String,
    #[serde(default)]
    delimiter: String,
    /// Emit a header row of column names (default true).
    #[serde(default = "default_true")]
    header: bool,
    #[serde(default)]
    null_value: String,
    #[serde(default)]
    quote: String,
    /// Prepend a UTF-8 BOM (default false).
    #[serde(default)]
    bom: bool,
}

fn default_true() -> bool {
    true
}

/// Single source for the chat schema (and CLI + page). Every param carries a
/// `.describe()`; every fixed-choice param is a `Param::enumv`.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("sql")
                .required()
                .describe("The SQL dump text. INSERT statements provide the rows; CREATE TABLE (if present) supplies column names. Comments (-- , #, /* */) and other statements are ignored."),
        )
        .param(
            Param::string("table")
                .describe("Export only this table (case-insensitive, unquoted name). Blank exports every table found, each in its own '### TABLE: name' section."),
        )
        .param(
            Param::enumv("delimiter", ["comma", "tab", "semicolon", "pipe"])
                .default("comma")
                .describe("Field separator for the output. One of: comma (CSV), tab (TSV), semicolon, or pipe. Default comma."),
        )
        .param(
            Param::boolean("header")
                .default(true)
                .describe("Include a first row of column names (from the INSERT column list, else CREATE TABLE, else col1..colN). Default true."),
        )
        .param(
            Param::string("null_value")
                .describe("Text to write for a SQL NULL cell. Default is an empty field; set to e.g. NULL or \\N to make nulls explicit."),
        )
        .param(
            Param::enumv("quote", ["minimal", "all"])
                .default("minimal")
                .describe("Quoting policy. 'minimal' (default) wraps a field in double quotes only when it contains the delimiter, a quote, or a newline; 'all' wraps every field."),
        )
        .param(
            Param::boolean("bom")
                .default(false)
                .describe("Prepend a UTF-8 byte-order mark so Excel opens the CSV as UTF-8. Default false."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/sql-dump-to-csv",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Extract the rows from a SQL dump's INSERT statements into CSV, one per table.",
    skill(
        description = "Extract the row data from the INSERT statements in a SQL dump and return it as RFC-4180 CSV, one CSV section per table. Column names come from the INSERT column list when present, else from a matching CREATE TABLE, else generated col1..colN. Handles multi-row INSERTs, doubled '' and MySQL backslash string escapes, backtick/double-quote/[bracket] identifiers, and -- / # / /* */ comments. Options: table (export just one table), delimiter (comma/tab/semicolon/pipe), header (column-name row on/off), null_value (text for SQL NULL), quote (minimal/all), bom (UTF-8 BOM for Excel).",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "sql-dump-to-csv", |a: Args| {
            gizza_ai_sql_dump_to_csv_core::convert(
                &a.sql,
                &a.table,
                &a.delimiter,
                a.header,
                &a.null_value,
                &a.quote,
                a.bom,
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

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema so any future change to the LLM-facing API is intentional.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "sql": { "type": "string", "description": "The SQL dump text. INSERT statements provide the rows; CREATE TABLE (if present) supplies column names. Comments (-- , #, /* */) and other statements are ignored." },
                    "table": { "type": "string", "description": "Export only this table (case-insensitive, unquoted name). Blank exports every table found, each in its own '### TABLE: name' section." },
                    "delimiter": { "type": "string", "enum": ["comma", "tab", "semicolon", "pipe"], "default": "comma", "description": "Field separator for the output. One of: comma (CSV), tab (TSV), semicolon, or pipe. Default comma." },
                    "header": { "type": "boolean", "default": true, "description": "Include a first row of column names (from the INSERT column list, else CREATE TABLE, else col1..colN). Default true." },
                    "null_value": { "type": "string", "description": "Text to write for a SQL NULL cell. Default is an empty field; set to e.g. NULL or \\N to make nulls explicit." },
                    "quote": { "type": "string", "enum": ["minimal", "all"], "default": "minimal", "description": "Quoting policy. 'minimal' (default) wraps a field in double quotes only when it contains the delimiter, a quote, or a newline; 'all' wraps every field." },
                    "bom": { "type": "boolean", "default": false, "description": "Prepend a UTF-8 byte-order mark so Excel opens the CSV as UTF-8. Default false." }
                },
                "required": ["sql"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
