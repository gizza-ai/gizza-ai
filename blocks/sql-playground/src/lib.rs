//! gizza-ai/sql-playground — run SQL against an in-memory database.
//! Thin chat-skill wrapper around the core; chat schema single-sourced from
//! descriptor() (also drives the CLI); handler delegates to run_skill. Pure.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    sql: String,
    #[serde(default)]
    format: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("sql")
                .required()
                .describe("One or more SQL statements separated by ';'. Runs against a fresh in-memory database (so include your CREATE TABLE / INSERT before the SELECT). Supports CREATE TABLE, INSERT, UPDATE, DELETE, SELECT with WHERE/ORDER BY/LIMIT/GROUP BY/JOIN and common functions. The result of the LAST statement is returned."),
        )
        .param(
            Param::enumv("format", ["table", "csv", "json"])
                .default("table")
                .describe("Output format for a SELECT result: 'table' (default) is an aligned ASCII grid, 'csv' is comma-separated with a header row, 'json' is an array of row objects. Non-SELECT statements report rows affected."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/sql-playground",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Run SQL against an in-memory database",
    skill(
        description = "Create an in-memory SQL database from your statements and run queries against it, returning the result of the last statement. Pass one or more semicolon-separated SQL statements in 'sql' (e.g. a CREATE TABLE and INSERTs followed by a SELECT) — the database is fresh each call, so include the schema and data you want to query. Supports CREATE TABLE, INSERT, UPDATE, DELETE and SELECT with WHERE, ORDER BY, LIMIT, GROUP BY, aggregate functions and JOINs. Use format='table' (default) for an aligned grid, 'csv' for comma-separated rows, or 'json' for an array of row objects.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "sql-playground", |a: Args| {
            let fmt = if a.format.is_empty() { "table" } else { &a.format };
            gizza_ai_sql_playground_core::run(&a.sql, fmt).map_err(SkillError::InvalidArgs)
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
                    "sql":    { "type": "string", "description": "One or more SQL statements separated by ';'. Runs against a fresh in-memory database (so include your CREATE TABLE / INSERT before the SELECT). Supports CREATE TABLE, INSERT, UPDATE, DELETE, SELECT with WHERE/ORDER BY/LIMIT/GROUP BY/JOIN and common functions. The result of the LAST statement is returned." },
                    "format": { "type": "string", "enum": ["table", "csv", "json"], "default": "table", "description": "Output format for a SELECT result: 'table' (default) is an aligned ASCII grid, 'csv' is comma-separated with a header row, 'json' is an array of row objects. Non-SELECT statements report rows affected." }
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
