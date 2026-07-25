//! gizza-ai/schema-inferrer — infer a JSON Schema AND a SQL `CREATE TABLE` from
//! a sample dataset (CSV/delimited text or JSON records).
//!
//! Thin chat-skill wrapper around `gizza-ai-schema-inferrer-core`. The chat
//! schema is derived from `descriptor()` (single source — shared shape across
//! chat + CLI); the handler delegates to `block_utils::run_skill`. No host calls
//! — runs entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default)]
    format: String,
    #[serde(default)]
    delimiter: String,
    #[serde(default = "default_true")]
    has_header: bool,
    #[serde(default)]
    output: String,
    #[serde(default)]
    table: String,
    #[serde(default)]
    dialect: String,
    #[serde(default)]
    draft: String,
    #[serde(default)]
    primary_key: String,
    #[serde(default = "default_true")]
    not_null: bool,
    #[serde(default = "default_true")]
    detect_formats: bool,
}

fn default_true() -> bool {
    true
}

/// Single-source param descriptor → chat schema (and CLI). See
/// docs/superpowers/specs/2026-06-19-gizza-shared-tool-abstraction-design.md.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("data")
                .required()
                .describe("The sample dataset to infer a schema from: CSV/delimited text with a header row, or JSON (an object = one record, or an array of record objects). Example CSV: \"id,name\\n1,Alice\\n2,Bob\". Column/field names come from the header (CSV) or object keys (JSON)."),
        )
        .param(
            Param::enumv("format", ["auto", "csv", "json"])
                .default("auto")
                .describe("How to parse the input. 'auto' (default) treats text starting with { or [ as JSON, otherwise CSV. Force 'csv' or 'json' to override. Default auto."),
        )
        .param(
            Param::enumv("delimiter", ["auto", "comma", "tab", "semicolon", "pipe"])
                .default("auto")
                .describe("CSV field delimiter. 'auto' (default) sniffs comma/semicolon/tab/pipe by picking the most consistent column count. Ignored for JSON input. Default auto."),
        )
        .param(
            Param::boolean("has_header")
                .default(true)
                .describe("Treat the first CSV row as column names. When false, columns are named column_1, column_2, … Ignored for JSON input (keys are always the columns). Default true."),
        )
        .param(
            Param::enumv("output", ["both", "json-schema", "sql"])
                .default("both")
                .describe("Which artifact to emit. 'both' (default) returns the JSON Schema followed by the SQL CREATE TABLE; 'json-schema' returns only the JSON Schema; 'sql' returns only the CREATE TABLE. Default both."),
        )
        .param(
            Param::string("table")
                .default("my_table")
                .describe("Table name for CREATE TABLE and the JSON Schema 'title' (default \"my_table\"). May be schema-qualified like \"public.users\" — each dot-separated part is quoted independently in the SQL."),
        )
        .param(
            Param::enumv("dialect", ["mysql", "postgres", "sqlite", "mssql", "ansi"])
                .default("postgres")
                .describe("SQL dialect for CREATE TABLE. Sets identifier quoting (mysql=`backticks`, postgres/sqlite/ansi=\"double quotes\", mssql=[brackets]) and column types (e.g. float → DOUBLE/DOUBLE PRECISION/REAL/FLOAT, boolean → BOOLEAN/TINYINT(1)/BIT, datetime → TIMESTAMP/DATETIME). Default postgres."),
        )
        .param(
            Param::enumv("draft", ["2020-12", "draft-07"])
                .default("2020-12")
                .describe("JSON Schema dialect to emit. '2020-12' (default) or 'draft-07'. Affects the $schema URL and whether a 'uuid' string format is emitted (Draft-07 has no uuid format). Default 2020-12."),
        )
        .param(
            Param::string("primary_key")
                .describe("Name of the column to mark as PRIMARY KEY in CREATE TABLE (and treat as required/NOT NULL in both outputs). Must be one of the columns. Optional."),
        )
        .param(
            Param::boolean("not_null")
                .default(true)
                .describe("Emit NOT NULL in CREATE TABLE (and list under JSON Schema 'required') for every column that had a value in every row. When false, columns are left nullable unless they are the primary key. Default true."),
        )
        .param(
            Param::boolean("detect_formats")
                .default(true)
                .describe("Tag text columns whose values all match a known JSON Schema string 'format' (email, uri, uuid, ipv4). Date/datetime columns always get date/date-time regardless. Default true."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct SchemaInferrer;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/schema-inferrer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Infer a JSON Schema and a SQL CREATE TABLE from a sample dataset.",
    skill(
        description = "From a sample dataset — CSV/delimited text or JSON records — infer a per-column type and nullability, then emit a standard JSON Schema (Draft 2020-12 or Draft-07) and/or a SQL CREATE TABLE statement. Type inference recognizes integer, float, boolean, date, and datetime columns (zero-padded codes like ZIP codes stay text); the JSON Schema maps them to integer/number/boolean/string (dates via the date/date-time format) and can tag email/uri/uuid/ipv4 string formats, while the CREATE TABLE maps them to the chosen dialect's column types (mysql, postgres, sqlite, mssql, ansi). Columns present in every record become required / NOT NULL; an optional PRIMARY KEY is marked in both outputs. Use the output option to get both artifacts, just the JSON Schema, or just the SQL. Emits schemas only — no INSERT rows.",
        parameters = schema_json()
    ),
)]
impl SchemaInferrer {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned value in { "result": … } and routes
        // errors through GuestResult::error.
        match run_skill(&body, "schema-inferrer", |a: Args| {
            gizza_ai_schema_inferrer_core::infer_from_str(
                &a.data,
                &a.format,
                &a.delimiter,
                a.has_header,
                &a.output,
                &a.table,
                &a.dialect,
                &a.draft,
                &a.primary_key,
                a.not_null,
                a.detect_formats,
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
    /// reviewed. Authored 2026-07-25 for the initial schema-inferrer release.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "data": { "type": "string", "description": "The sample dataset to infer a schema from: CSV/delimited text with a header row, or JSON (an object = one record, or an array of record objects). Example CSV: \"id,name\\n1,Alice\\n2,Bob\". Column/field names come from the header (CSV) or object keys (JSON)." },
                    "format": { "type": "string", "enum": ["auto", "csv", "json"], "default": "auto", "description": "How to parse the input. 'auto' (default) treats text starting with { or [ as JSON, otherwise CSV. Force 'csv' or 'json' to override. Default auto." },
                    "delimiter": { "type": "string", "enum": ["auto", "comma", "tab", "semicolon", "pipe"], "default": "auto", "description": "CSV field delimiter. 'auto' (default) sniffs comma/semicolon/tab/pipe by picking the most consistent column count. Ignored for JSON input. Default auto." },
                    "has_header": { "type": "boolean", "default": true, "description": "Treat the first CSV row as column names. When false, columns are named column_1, column_2, … Ignored for JSON input (keys are always the columns). Default true." },
                    "output": { "type": "string", "enum": ["both", "json-schema", "sql"], "default": "both", "description": "Which artifact to emit. 'both' (default) returns the JSON Schema followed by the SQL CREATE TABLE; 'json-schema' returns only the JSON Schema; 'sql' returns only the CREATE TABLE. Default both." },
                    "table": { "type": "string", "default": "my_table", "description": "Table name for CREATE TABLE and the JSON Schema 'title' (default \"my_table\"). May be schema-qualified like \"public.users\" — each dot-separated part is quoted independently in the SQL." },
                    "dialect": { "type": "string", "enum": ["mysql", "postgres", "sqlite", "mssql", "ansi"], "default": "postgres", "description": "SQL dialect for CREATE TABLE. Sets identifier quoting (mysql=`backticks`, postgres/sqlite/ansi=\"double quotes\", mssql=[brackets]) and column types (e.g. float → DOUBLE/DOUBLE PRECISION/REAL/FLOAT, boolean → BOOLEAN/TINYINT(1)/BIT, datetime → TIMESTAMP/DATETIME). Default postgres." },
                    "draft": { "type": "string", "enum": ["2020-12", "draft-07"], "default": "2020-12", "description": "JSON Schema dialect to emit. '2020-12' (default) or 'draft-07'. Affects the $schema URL and whether a 'uuid' string format is emitted (Draft-07 has no uuid format). Default 2020-12." },
                    "primary_key": { "type": "string", "description": "Name of the column to mark as PRIMARY KEY in CREATE TABLE (and treat as required/NOT NULL in both outputs). Must be one of the columns. Optional." },
                    "not_null": { "type": "boolean", "default": true, "description": "Emit NOT NULL in CREATE TABLE (and list under JSON Schema 'required') for every column that had a value in every row. When false, columns are left nullable unless they are the primary key. Default true." },
                    "detect_formats": { "type": "boolean", "default": true, "description": "Tag text columns whose values all match a known JSON Schema string 'format' (email, uri, uuid, ipv4). Date/datetime columns always get date/date-time regardless. Default true." }
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
