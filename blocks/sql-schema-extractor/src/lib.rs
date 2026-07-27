//! gizza-ai/sql-schema-extractor — chat skill block on the shared tool
//! abstraction. Parses the DDL out of a SQL dump (`CREATE TABLE`, `ALTER TABLE`,
//! `CREATE INDEX`) into a clean table/column/constraint model rendered as JSON
//! or Markdown. The chat schema is single-sourced from `descriptor()` (which
//! also drives the CLI); `handle()` delegates to `block_utils::run_skill`. No
//! host calls — runs entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    sql: String,
    #[serde(default)]
    output: String,
    #[serde(default)]
    dialect: String,
    #[serde(default = "default_true")]
    apply_alter: bool,
    #[serde(default = "default_true")]
    include_indexes: bool,
}

fn default_true() -> bool {
    true
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("sql")
                .required()
                .describe("The SQL dump / DDL text to parse. Include one or more CREATE TABLE (and optionally ALTER TABLE / CREATE INDEX) statements — e.g. \"CREATE TABLE users (id INT PRIMARY KEY, email VARCHAR(255) NOT NULL UNIQUE);\". Comments (-- , #, /* */), INSERT/SELECT and other non-DDL statements are ignored. Nothing is executed."),
        )
        .param(
            Param::enumv("output", ["json", "markdown"])
                .default("json")
                .describe("Output format. 'json' (default) emits a structured model {dialect, table_count, column_count, tables:[{name, columns, primary_key, foreign_keys, unique_constraints, checks, indexes}]}. 'markdown' emits one section per table with a column table plus bullet lists of the constraints. Default json."),
        )
        .param(
            Param::enumv("dialect", ["auto", "mysql", "postgres", "sqlite", "mssql", "generic"])
                .default("auto")
                .describe("SQL dialect hint. Identifiers are normalized for every dialect (backticks, \"double quotes\" and [brackets] are stripped); this mainly controls whether '#' starts a line comment (mysql/auto) and is echoed back in the output. Default auto."),
        )
        .param(
            Param::boolean("apply_alter")
                .default(true)
                .describe("Fold ALTER TABLE ... ADD [COLUMN|CONSTRAINT|PRIMARY KEY|FOREIGN KEY|UNIQUE|CHECK|INDEX] statements onto their target table so the model reflects the final schema. When false, ALTER statements are ignored and only CREATE TABLE is used. Default true."),
        )
        .param(
            Param::boolean("include_indexes")
                .default(true)
                .describe("Include index definitions (table-level KEY/INDEX, plus CREATE INDEX / CREATE UNIQUE INDEX) in the model. When false, indexes are omitted (the 'indexes' array is dropped from JSON and the index bullets from Markdown). Default true."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/sql-schema-extractor",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Parse CREATE TABLE / ALTER TABLE from a SQL dump into a clean table/column/constraint model.",
    skill(
        description = "Parse the DDL out of a SQL dump — CREATE TABLE, ALTER TABLE, and CREATE INDEX statements — into a clean, structured schema model. For each table it extracts columns (name, data type with params like VARCHAR(255)/DECIMAL(10,2), nullability, default value, auto-increment/serial/identity), the primary key (inline or table-level, including composite), foreign keys (columns, referenced table/columns, ON DELETE/UPDATE actions), unique constraints, CHECK constraints, and indexes. ALTER TABLE ... ADD statements are folded onto their target table so the model reflects the final schema. It is a lenient parser, not a SQL engine: comments and non-DDL statements (INSERT, SELECT, DROP, …) are skipped, and nothing is executed. Output as a JSON model or a Markdown schema doc. Supports MySQL, PostgreSQL, SQLite, MSSQL and generic dialects (identifier quoting normalized). It never emits INSERT rows.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "sql-schema-extractor", |a: Args| {
            gizza_ai_sql_schema_extractor_core::extract(
                &a.sql,
                &a.output,
                &a.dialect,
                a.apply_alter,
                a.include_indexes,
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
    /// schema, so any future change to the LLM-facing API is intentional and
    /// reviewed. Authored 2026-07-27 for the initial sql-schema-extractor release.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "sql": { "type": "string", "description": "The SQL dump / DDL text to parse. Include one or more CREATE TABLE (and optionally ALTER TABLE / CREATE INDEX) statements — e.g. \"CREATE TABLE users (id INT PRIMARY KEY, email VARCHAR(255) NOT NULL UNIQUE);\". Comments (-- , #, /* */), INSERT/SELECT and other non-DDL statements are ignored. Nothing is executed." },
                    "output": { "type": "string", "enum": ["json", "markdown"], "default": "json", "description": "Output format. 'json' (default) emits a structured model {dialect, table_count, column_count, tables:[{name, columns, primary_key, foreign_keys, unique_constraints, checks, indexes}]}. 'markdown' emits one section per table with a column table plus bullet lists of the constraints. Default json." },
                    "dialect": { "type": "string", "enum": ["auto", "mysql", "postgres", "sqlite", "mssql", "generic"], "default": "auto", "description": "SQL dialect hint. Identifiers are normalized for every dialect (backticks, \"double quotes\" and [brackets] are stripped); this mainly controls whether '#' starts a line comment (mysql/auto) and is echoed back in the output. Default auto." },
                    "apply_alter": { "type": "boolean", "default": true, "description": "Fold ALTER TABLE ... ADD [COLUMN|CONSTRAINT|PRIMARY KEY|FOREIGN KEY|UNIQUE|CHECK|INDEX] statements onto their target table so the model reflects the final schema. When false, ALTER statements are ignored and only CREATE TABLE is used. Default true." },
                    "include_indexes": { "type": "boolean", "default": true, "description": "Include index definitions (table-level KEY/INDEX, plus CREATE INDEX / CREATE UNIQUE INDEX) in the model. When false, indexes are omitted (the 'indexes' array is dropped from JSON and the index bullets from Markdown). Default true." }
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
