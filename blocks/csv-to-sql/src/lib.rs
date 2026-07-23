//! gizza-ai/csv-to-sql — turn a CSV (or JSON) table into SQL `CREATE TABLE` +
//! `INSERT` statements with per-column types inferred from the data.
//!
//! Thin chat-skill wrapper around `gizza-ai-csv-to-sql-core`. The chat schema is
//! derived from `descriptor()` (single source — shared shape across chat + CLI);
//! the handler delegates to `block_utils::run_skill`. No host calls — runs
//! entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default)]
    format: String,
    #[serde(default)]
    delimiter: String,
    #[serde(default = "default_true")]
    has_header: bool,
    #[serde(default)]
    table: String,
    #[serde(default)]
    dialect: String,
    #[serde(default)]
    values: String,
    #[serde(default = "default_true")]
    multi_row: bool,
    #[serde(default = "default_true")]
    create_table: bool,
    #[serde(default)]
    drop_table: bool,
    #[serde(default)]
    primary_key: String,
    #[serde(default = "default_true")]
    quote_identifiers: bool,
    #[serde(default)]
    null_handling: String,
    #[serde(default = "default_true")]
    infer_types: bool,
    #[serde(default = "default_true")]
    detect_dates: bool,
}

fn default_true() -> bool {
    true
}

/// Single-source param descriptor → chat schema (and CLI). See
/// docs/superpowers/specs/2026-06-19-gizza-shared-tool-abstraction-design.md.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("The table data: CSV/delimited text with a header row, or JSON (an object = one row, or an array of row objects). Example CSV: \"id,name\\n1,Alice\\n2,Bob\". Column names come from the header (CSV) or object keys (JSON)."),
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
            Param::string("table")
                .default("my_table")
                .describe("Target table name (default \"my_table\"). May be schema-qualified like \"public.users\" — each dot-separated part is quoted independently."),
        )
        .param(
            Param::enumv("dialect", ["mysql", "postgres", "sqlite", "mssql", "ansi"])
                .default("mysql")
                .describe("SQL dialect. Sets identifier quoting (mysql=`backticks`, postgres/sqlite/ansi=\"double quotes\", mssql=[brackets]), boolean literals (postgres/ansi=TRUE/FALSE, others=1/0), string escaping (mysql also escapes backslashes), placeholder syntax, and CREATE TABLE column types (e.g. dates → DATE, datetimes → TIMESTAMP/DATETIME). Default mysql."),
        )
        .param(
            Param::enumv("values", ["literal", "placeholder"])
                .default("literal")
                .describe("Value output mode. 'literal' (default) inlines escaped SQL literals. 'placeholder' emits positional placeholders for a prepared statement (? for mysql/sqlite, @pN for mssql, $N for postgres) and lists the bound values in a trailing '-- params:' comment. Default literal."),
        )
        .param(
            Param::boolean("multi_row")
                .default(true)
                .describe("Emit one multi-row INSERT ... VALUES (...),(...) instead of a separate INSERT per row. Default true."),
        )
        .param(
            Param::boolean("create_table")
                .default(true)
                .describe("Emit a CREATE TABLE before the inserts, with each column's type inferred from the data (integer/float/boolean/date/datetime/text). Default true."),
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
                .describe("How a blank cell (or JSON null / missing key) is written: 'null' → NULL (default), 'default' → the SQL keyword DEFAULT (use the column's own default), 'empty-string' → an empty string ''. Default null."),
        )
        .param(
            Param::boolean("infer_types")
                .default(true)
                .describe("Infer a type per column (integer/float/boolean/date/datetime/text) from the values, driving CREATE TABLE column types and whether numbers/booleans are emitted unquoted. When false, every column is text and every value is a quoted string. Default true."),
        )
        .param(
            Param::boolean("detect_dates")
                .default(true)
                .describe("When inferring types, also recognize date (YYYY-MM-DD, MM/DD/YYYY, …) and datetime (YYYY-MM-DD HH:MM:SS, ISO 8601) columns and map them to SQL DATE/TIMESTAMP types. When false, date-looking columns stay text. Only used when infer_types is true. Default true."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct CsvToSql;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/csv-to-sql",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Generate SQL CREATE TABLE and INSERT statements from a CSV or JSON table.",
    skill(
        description = "Turn a CSV (or JSON) table into runnable SQL: a CREATE TABLE with a type inferred for every column plus INSERT statements. Accepts delimited text with a header row (delimiter auto-sniffed, or set comma/tab/semicolon/pipe) or JSON (an object = one row, an array = one row per object). Type inference recognizes integer, float, boolean, date, and datetime columns (zero-padded codes like ZIP codes stay text) and maps them to the chosen dialect's SQL types (mysql, postgres, sqlite, mssql, ansi). Pick literal or placeholder/prepared-statement values, one multi-row INSERT or per-row inserts, an optional PRIMARY KEY and DROP TABLE IF EXISTS, and how blank cells become NULL/DEFAULT/''.",
        parameters = schema_json()
    ),
)]
impl CsvToSql {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned value in { "result": … } and routes
        // errors through GuestResult::error.
        match run_skill(&body, "csv-to-sql", |a: Args| {
            gizza_ai_csv_to_sql_core::generate_from_str(
                &a.input,
                &a.format,
                &a.delimiter,
                a.has_header,
                &a.table,
                &a.dialect,
                &a.values,
                a.multi_row,
                a.create_table,
                a.drop_table,
                &a.primary_key,
                a.quote_identifiers,
                &a.null_handling,
                a.infer_types,
                a.detect_dates,
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
    /// reviewed. Authored 2026-07-23 for the initial csv-to-sql release.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "input": { "type": "string", "description": "The table data: CSV/delimited text with a header row, or JSON (an object = one row, or an array of row objects). Example CSV: \"id,name\\n1,Alice\\n2,Bob\". Column names come from the header (CSV) or object keys (JSON)." },
                    "format": { "type": "string", "enum": ["auto", "csv", "json"], "default": "auto", "description": "How to parse the input. 'auto' (default) treats text starting with { or [ as JSON, otherwise CSV. Force 'csv' or 'json' to override. Default auto." },
                    "delimiter": { "type": "string", "enum": ["auto", "comma", "tab", "semicolon", "pipe"], "default": "auto", "description": "CSV field delimiter. 'auto' (default) sniffs comma/semicolon/tab/pipe by picking the most consistent column count. Ignored for JSON input. Default auto." },
                    "has_header": { "type": "boolean", "default": true, "description": "Treat the first CSV row as column names. When false, columns are named column_1, column_2, … Ignored for JSON input (keys are always the columns). Default true." },
                    "table": { "type": "string", "default": "my_table", "description": "Target table name (default \"my_table\"). May be schema-qualified like \"public.users\" — each dot-separated part is quoted independently." },
                    "dialect": { "type": "string", "enum": ["mysql", "postgres", "sqlite", "mssql", "ansi"], "default": "mysql", "description": "SQL dialect. Sets identifier quoting (mysql=`backticks`, postgres/sqlite/ansi=\"double quotes\", mssql=[brackets]), boolean literals (postgres/ansi=TRUE/FALSE, others=1/0), string escaping (mysql also escapes backslashes), placeholder syntax, and CREATE TABLE column types (e.g. dates → DATE, datetimes → TIMESTAMP/DATETIME). Default mysql." },
                    "values": { "type": "string", "enum": ["literal", "placeholder"], "default": "literal", "description": "Value output mode. 'literal' (default) inlines escaped SQL literals. 'placeholder' emits positional placeholders for a prepared statement (? for mysql/sqlite, @pN for mssql, $N for postgres) and lists the bound values in a trailing '-- params:' comment. Default literal." },
                    "multi_row": { "type": "boolean", "default": true, "description": "Emit one multi-row INSERT ... VALUES (...),(...) instead of a separate INSERT per row. Default true." },
                    "create_table": { "type": "boolean", "default": true, "description": "Emit a CREATE TABLE before the inserts, with each column's type inferred from the data (integer/float/boolean/date/datetime/text). Default true." },
                    "drop_table": { "type": "boolean", "default": false, "description": "Prepend a DROP TABLE IF EXISTS statement. Default false." },
                    "primary_key": { "type": "string", "description": "Name of the column to mark as PRIMARY KEY in the generated CREATE TABLE (only used when create_table is true). Must be one of the columns. Optional." },
                    "quote_identifiers": { "type": "boolean", "default": true, "description": "Quote table and column identifiers for the dialect. When false, identifiers are emitted bare and validated as safe (letters, digits, underscores; not starting with a digit) — an unsafe name errors. Default true." },
                    "null_handling": { "type": "string", "enum": ["null", "default", "empty-string"], "default": "null", "description": "How a blank cell (or JSON null / missing key) is written: 'null' → NULL (default), 'default' → the SQL keyword DEFAULT (use the column's own default), 'empty-string' → an empty string ''. Default null." },
                    "infer_types": { "type": "boolean", "default": true, "description": "Infer a type per column (integer/float/boolean/date/datetime/text) from the values, driving CREATE TABLE column types and whether numbers/booleans are emitted unquoted. When false, every column is text and every value is a quoted string. Default true." },
                    "detect_dates": { "type": "boolean", "default": true, "description": "When inferring types, also recognize date (YYYY-MM-DD, MM/DD/YYYY, …) and datetime (YYYY-MM-DD HH:MM:SS, ISO 8601) columns and map them to SQL DATE/TIMESTAMP types. When false, date-looking columns stay text. Only used when infer_types is true. Default true." }
                },
                "required": ["input"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
