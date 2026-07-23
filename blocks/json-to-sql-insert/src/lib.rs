//! gizza-ai/json-to-sql-insert — turn a JSON object or an array of row objects
//! into SQL `INSERT` statements (optionally with a `CREATE TABLE`).
//!
//! Thin chat-skill wrapper around `gizza-ai-json-to-sql-insert-core`. The chat
//! schema is derived from `descriptor()` (single source — shared shape across
//! chat + CLI); the handler delegates to `block_utils::run_skill`. No host
//! calls — runs entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    json: String,
    #[serde(default)]
    table: String,
    #[serde(default)]
    dialect: String,
    #[serde(default)]
    values: String,
    #[serde(default = "default_true")]
    multi_row: bool,
    #[serde(default)]
    create_table: bool,
    #[serde(default)]
    drop_table: bool,
    #[serde(default)]
    primary_key: String,
    #[serde(default = "default_true")]
    quote_identifiers: bool,
    #[serde(default)]
    null_handling: String,
    #[serde(default)]
    sort_columns: bool,
}

fn default_true() -> bool {
    true
}

/// Single-source param descriptor → chat schema (and CLI). See
/// docs/superpowers/specs/2026-06-19-gizza-shared-tool-abstraction-design.md.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("json")
                .required()
                .describe("The data: a JSON object (one row) or an array of objects (one row each). Keys become columns; the column list is the union of all row keys. A key missing from a row is treated as null. Example: [{\"id\":1,\"name\":\"Alice\"},{\"id\":2,\"name\":\"Bob\"}]."),
        )
        .param(
            Param::string("table")
                .default("my_table")
                .describe("Target table name (default \"my_table\"). May be schema-qualified like \"public.users\" — each dot-separated part is quoted independently."),
        )
        .param(
            Param::enumv("dialect", ["mysql", "postgres", "sqlite", "mssql", "ansi"])
                .default("mysql")
                .describe("SQL dialect. Sets identifier quoting (mysql=`backticks`, postgres/sqlite/ansi=\"double quotes\", mssql=[brackets]), boolean literals (postgres/ansi=TRUE/FALSE, others=1/0), string escaping (mysql also escapes backslashes), placeholder syntax, and CREATE TABLE column types. Default mysql."),
        )
        .param(
            Param::enumv("values", ["literal", "placeholder"])
                .default("literal")
                .describe("Value output mode. 'literal' (default) inlines escaped SQL literals. 'placeholder' emits positional placeholders for a prepared statement (? for mysql/sqlite/mssql-as-@pN, $N for postgres) and lists the bound values in a trailing '-- params:' comment. Default literal."),
        )
        .param(
            Param::boolean("multi_row")
                .default(true)
                .describe("Emit one multi-row INSERT ... VALUES (...),(...) instead of a separate INSERT per row. Default true."),
        )
        .param(
            Param::boolean("create_table")
                .default(false)
                .describe("Also emit a CREATE TABLE before the inserts, with column types inferred from the data (integer/float/boolean/text). Default false."),
        )
        .param(
            Param::boolean("drop_table")
                .default(false)
                .describe("Prepend a DROP TABLE IF EXISTS statement. Default false."),
        )
        .param(
            Param::string("primary_key")
                .describe("Name of the column to mark as PRIMARY KEY in the generated CREATE TABLE (only used when create_table is true). Must be one of the columns. Optional."),
        )
        .param(
            Param::boolean("quote_identifiers")
                .default(true)
                .describe("Quote table and column identifiers for the dialect. When false, identifiers are emitted bare and validated as safe (letters, digits, underscores; not starting with a digit) — an unsafe name errors. Default true."),
        )
        .param(
            Param::enumv("null_handling", ["null", "default", "empty-string"])
                .default("null")
                .describe("How a JSON null (or a column missing from a row) is written: 'null' → NULL (default), 'default' → the SQL keyword DEFAULT (use the column's own default), 'empty-string' → an empty string ''. Default null."),
        )
        .param(
            Param::boolean("sort_columns")
                .default(false)
                .describe("Order columns alphabetically instead of by first appearance in the input. Default false (first-seen order)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct JsonToSqlInsert;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/json-to-sql-insert",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Generate SQL INSERT statements from JSON rows.",
    skill(
        description = "Turn a JSON object or an array of row objects into SQL INSERT statements. Keys become columns (the union of all rows' keys, in first-seen order unless sort_columns=true); a key missing from a row is treated as null. Pick a dialect (mysql, postgres, sqlite, mssql, ansi) to control identifier quoting, boolean/string literals, and placeholder syntax. Set values='placeholder' for parameterized/prepared-statement output (? or $N) with the bound values in a trailing comment. multi_row=true (default) batches every row into one INSERT ... VALUES (...),(...); false emits one INSERT per row. Optionally emit a CREATE TABLE (create_table=true, with inferred column types and an optional primary_key) and a DROP TABLE IF EXISTS (drop_table=true). null_handling chooses NULL, DEFAULT, or '' for nulls; quote_identifiers can be turned off for bare, validated identifiers.",
        parameters = schema_json()
    )
)]
impl JsonToSqlInsert {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned value in { "result": … } and routes
        // errors through GuestResult::error.
        match run_skill(&body, "json-to-sql-insert", |a: Args| {
            gizza_ai_json_to_sql_insert_core::generate_from_str(
                &a.json,
                &a.table,
                &a.dialect,
                &a.values,
                a.multi_row,
                a.create_table,
                a.drop_table,
                &a.primary_key,
                a.quote_identifiers,
                &a.null_handling,
                a.sort_columns,
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
    /// reviewed. Authored 2026-07-23 for the initial json-to-sql-insert release.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "json": { "type": "string", "description": "The data: a JSON object (one row) or an array of objects (one row each). Keys become columns; the column list is the union of all row keys. A key missing from a row is treated as null. Example: [{\"id\":1,\"name\":\"Alice\"},{\"id\":2,\"name\":\"Bob\"}]." },
                    "table": { "type": "string", "default": "my_table", "description": "Target table name (default \"my_table\"). May be schema-qualified like \"public.users\" — each dot-separated part is quoted independently." },
                    "dialect": { "type": "string", "enum": ["mysql", "postgres", "sqlite", "mssql", "ansi"], "default": "mysql", "description": "SQL dialect. Sets identifier quoting (mysql=`backticks`, postgres/sqlite/ansi=\"double quotes\", mssql=[brackets]), boolean literals (postgres/ansi=TRUE/FALSE, others=1/0), string escaping (mysql also escapes backslashes), placeholder syntax, and CREATE TABLE column types. Default mysql." },
                    "values": { "type": "string", "enum": ["literal", "placeholder"], "default": "literal", "description": "Value output mode. 'literal' (default) inlines escaped SQL literals. 'placeholder' emits positional placeholders for a prepared statement (? for mysql/sqlite/mssql-as-@pN, $N for postgres) and lists the bound values in a trailing '-- params:' comment. Default literal." },
                    "multi_row": { "type": "boolean", "default": true, "description": "Emit one multi-row INSERT ... VALUES (...),(...) instead of a separate INSERT per row. Default true." },
                    "create_table": { "type": "boolean", "default": false, "description": "Also emit a CREATE TABLE before the inserts, with column types inferred from the data (integer/float/boolean/text). Default false." },
                    "drop_table": { "type": "boolean", "default": false, "description": "Prepend a DROP TABLE IF EXISTS statement. Default false." },
                    "primary_key": { "type": "string", "description": "Name of the column to mark as PRIMARY KEY in the generated CREATE TABLE (only used when create_table is true). Must be one of the columns. Optional." },
                    "quote_identifiers": { "type": "boolean", "default": true, "description": "Quote table and column identifiers for the dialect. When false, identifiers are emitted bare and validated as safe (letters, digits, underscores; not starting with a digit) — an unsafe name errors. Default true." },
                    "null_handling": { "type": "string", "enum": ["null", "default", "empty-string"], "default": "null", "description": "How a JSON null (or a column missing from a row) is written: 'null' → NULL (default), 'default' → the SQL keyword DEFAULT (use the column's own default), 'empty-string' → an empty string ''. Default null." },
                    "sort_columns": { "type": "boolean", "default": false, "description": "Order columns alphabetically instead of by first appearance in the input. Default false (first-seen order)." }
                },
                "required": ["json"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
