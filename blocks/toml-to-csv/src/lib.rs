//! gizza-ai/toml-to-csv — chat skill block on the shared tool abstraction.
//! Turns a TOML array-of-tables into a CSV table (one row per entry, unioned
//! header). The chat schema is single-sourced from descriptor() (which also
//! drives the CLI); handle() delegates to block_utils::run_skill. Pure compute,
//! no host calls — runs entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

fn default_table() -> String {
    String::new()
}
fn default_nested() -> String {
    "flatten".to_string()
}
fn default_array_format() -> String {
    "json".to_string()
}
fn default_columns() -> String {
    "union".to_string()
}
fn default_delimiter() -> String {
    "comma".to_string()
}
fn default_include_header() -> bool {
    true
}

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default = "default_table")]
    table: String,
    #[serde(default = "default_nested")]
    nested: String,
    #[serde(default = "default_array_format")]
    array_format: String,
    #[serde(default = "default_columns")]
    columns: String,
    #[serde(default = "default_delimiter")]
    delimiter: String,
    #[serde(default = "default_include_header")]
    include_header: bool,
}

/// Single source for the chat schema (and CLI). Param order mirrors the core
/// `convert` signature and the web `run` export so all surfaces stay in sync.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("The TOML document to convert, e.g. a file with repeated [[users]] sections. Each array-of-tables entry becomes one CSV row and each key becomes a column."),
        )
        .param(
            Param::string("table")
                .default("")
                .describe("Dotted path of the array-of-tables to convert, e.g. 'users' or 'servers.pool'. Leave empty to auto-detect: the first root-level array-of-tables wins, then nested ones; a document with none is emitted as a single row. Quoted TOML keys are not supported in this path."),
        )
        .param(
            Param::enumv("nested", ["flatten", "json", "skip"])
                .default("flatten")
                .describe("How to render a nested table inside a row: flatten makes dot-notation columns (address.city), json puts the whole subtable in one cell as JSON, skip drops it. Default flatten."),
        )
        .param(
            Param::enumv("array_format", ["json", "join", "columns"])
                .default("json")
                .describe("How to render an array value inside a row: json writes [\"a\",\"b\"] in one cell, join writes 'a; b' in one cell, columns expands to 1-based indexed columns (tags.1, tags.2). Default json."),
        )
        .param(
            Param::enumv("columns", ["union", "sorted", "first"])
                .default("union")
                .describe("Header columns: union takes every key seen in any row in first-seen document order, sorted takes that same set alphabetically, first locks the header to the first row's keys and drops later extras. Default union."),
        )
        .param(
            Param::enumv("delimiter", ["comma", "semicolon", "tab", "pipe"])
                .default("comma")
                .describe("CSV field separator for the output. Values containing the separator, a quote, or a newline are RFC 4180 quoted. Default comma."),
        )
        .param(
            Param::boolean("include_header")
                .default(true)
                .describe("Write the header row of column names. Set false to append the rows to an existing sheet. Default true."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/toml-to-csv",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Turn a TOML array-of-tables into CSV rows with a unioned header",
    skill(
        description = "Convert a TOML document into CSV: each array-of-tables entry (e.g. every [[users]] section) becomes one row, and the header is the union of keys seen across entries so ragged entries keep their data. Pass the TOML text as 'input'. Leave 'table' empty to auto-detect the array-of-tables (first root-level one wins), or give a dotted path like 'servers.pool'; a document with no array-of-tables is emitted as a single row. nested=flatten (default) turns a nested table into dot-notation columns such as address.city, nested=json puts it in one cell as JSON, nested=skip drops it. array_format controls array values: json ([\"a\",\"b\"] in one cell), join ('a; b'), or columns (1-based tags.1, tags.2). columns picks the header order: union (first-seen document order), sorted (alphabetical), or first (the first row's keys only). delimiter is comma/semicolon/tab/pipe and include_header toggles the header row. Values are RFC 4180 quoted; dates and numbers keep their TOML text form. Comments are not preserved (TOML parsing drops them). Up to 20,000 rows and 2,000 columns per call. Runs locally; nothing is uploaded.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned value in { "result": ... }.
        match run_skill(&body, "toml-to-csv", |a: Args| {
            gizza_ai_toml_to_csv_core::convert(
                &a.input,
                &a.table,
                &a.nested,
                &a.array_format,
                &a.columns,
                &a.delimiter,
                a.include_header,
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
    /// reviewed.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "input": { "type": "string", "description": "The TOML document to convert, e.g. a file with repeated [[users]] sections. Each array-of-tables entry becomes one CSV row and each key becomes a column." },
                    "table": { "type": "string", "default": "", "description": "Dotted path of the array-of-tables to convert, e.g. 'users' or 'servers.pool'. Leave empty to auto-detect: the first root-level array-of-tables wins, then nested ones; a document with none is emitted as a single row. Quoted TOML keys are not supported in this path." },
                    "nested": { "type": "string", "enum": ["flatten", "json", "skip"], "default": "flatten", "description": "How to render a nested table inside a row: flatten makes dot-notation columns (address.city), json puts the whole subtable in one cell as JSON, skip drops it. Default flatten." },
                    "array_format": { "type": "string", "enum": ["json", "join", "columns"], "default": "json", "description": "How to render an array value inside a row: json writes [\"a\",\"b\"] in one cell, join writes 'a; b' in one cell, columns expands to 1-based indexed columns (tags.1, tags.2). Default json." },
                    "columns": { "type": "string", "enum": ["union", "sorted", "first"], "default": "union", "description": "Header columns: union takes every key seen in any row in first-seen document order, sorted takes that same set alphabetically, first locks the header to the first row's keys and drops later extras. Default union." },
                    "delimiter": { "type": "string", "enum": ["comma", "semicolon", "tab", "pipe"], "default": "comma", "description": "CSV field separator for the output. Values containing the separator, a quote, or a newline are RFC 4180 quoted. Default comma." },
                    "include_header": { "type": "boolean", "default": true, "description": "Write the header row of column names. Set false to append the rows to an existing sheet. Default true." }
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
