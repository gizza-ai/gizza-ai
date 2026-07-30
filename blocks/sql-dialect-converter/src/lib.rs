//! gizza-ai/sql-dialect-converter — chat skill block on the shared tool abstraction.
//! Converts SQL between PostgreSQL, MySQL and SQLite (identifier quoting,
//! auto-increment columns, CREATE TABLE data types, MySQL table options). The
//! chat schema is single-sourced from descriptor() (which also drives the CLI);
//! handle() delegates to block_utils::run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_sql_dialect_converter_core::{convert, Dialect};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    sql: String,
    from: String,
    to: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("sql")
                .required()
                .describe("The SQL to convert (DDL and/or queries). e.g. CREATE TABLE \"users\" (id SERIAL PRIMARY KEY)."),
        )
        .param(
            Param::enumv("from", ["postgres", "mysql", "sqlite"])
                .required()
                .describe("Source SQL dialect: postgres, mysql, or sqlite."),
        )
        .param(
            Param::enumv("to", ["postgres", "mysql", "sqlite"])
                .required()
                .describe("Target SQL dialect to convert to: postgres, mysql, or sqlite. Types are mapped to this dialect; from==to returns the input unchanged."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/sql-dialect-converter",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert SQL between PostgreSQL, MySQL and SQLite",
    skill(
        description = "Convert SQL between PostgreSQL, MySQL and SQLite. Rewrites delimited-identifier quoting (\"x\"/`x`/[x]), auto-increment columns (SERIAL / AUTO_INCREMENT / INTEGER PRIMARY KEY AUTOINCREMENT), CREATE TABLE column data types (bool, varchar, timestamp, blob, double, json, uuid, …), and strips MySQL table options (ENGINE=…, DEFAULT CHARSET=…) when the target isn't MySQL. String literals and comments are preserved. Set from and to (postgres/mysql/sqlite). Uses a forgiving tokenizer, not a full grammar — expression/function rewriting, procedures and views are out of scope. Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "sql-dialect-converter", |a: Args| {
            let from = Dialect::parse(&a.from).map_err(SkillError::InvalidArgs)?;
            let to = Dialect::parse(&a.to).map_err(SkillError::InvalidArgs)?;
            convert(&a.sql, from, to).map_err(SkillError::InvalidArgs)
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
                    "sql":  { "type": "string", "description": "The SQL to convert (DDL and/or queries). e.g. CREATE TABLE \"users\" (id SERIAL PRIMARY KEY)." },
                    "from": { "type": "string", "enum": ["postgres", "mysql", "sqlite"], "description": "Source SQL dialect: postgres, mysql, or sqlite." },
                    "to":   { "type": "string", "enum": ["postgres", "mysql", "sqlite"], "description": "Target SQL dialect to convert to: postgres, mysql, or sqlite. Types are mapped to this dialect; from==to returns the input unchanged." }
                },
                "required": ["sql", "from", "to"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
