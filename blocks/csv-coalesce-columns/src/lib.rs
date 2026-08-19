//! gizza-ai/csv-coalesce-columns — build one column from the first non-empty
//! value across a priority-ordered list of source columns (SQL COALESCE over
//! columns), optionally dropping the sources. Thin wrapper around the core; chat
//! schema single-sourced from descriptor(); handler delegates to run_skill. Pure.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_csv_coalesce_columns_core::coalesce_columns;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    columns: String,
    #[serde(default)]
    output: String,
    #[serde(default)]
    position: String,
    #[serde(default)]
    fallback: String,
    #[serde(default)]
    drop_sources: bool,
    #[serde(default = "default_true")]
    blank_is_empty: bool,
    #[serde(default)]
    null_tokens: String,
    #[serde(default = "default_true")]
    header: bool,
    #[serde(default)]
    delimiter: String,
}
fn default_true() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe("The CSV text to process, e.g. 'name,mobile,office\\nAnn,,555-2'. Rows shorter than the header are padded with empty cells."))
        .param(Param::string("columns").required().describe("Comma-separated source columns in PRIORITY order — the first one that has a value wins. Use header names when header=true (e.g. 'mobile,office,home') or 1-based indices (e.g. '2,3,4')."))
        .param(Param::string("output").default("").describe("Name for the new coalesced column. Default '' = 'coalesced'. It must not collide with a column you keep."))
        .param(Param::enumv("position", ["end", "start", "first-source"]).default("end").describe("Where the new column goes: 'end' appends it after the last column (default), 'start' puts it first, 'first-source' puts it where the first listed source column sat."))
        .param(Param::string("fallback").default("").describe("Value written when EVERY source column is empty for that row, e.g. 'N/A' or 'unknown'. Default '' leaves the cell blank."))
        .param(Param::boolean("drop_sources").default(false).describe("Remove the source columns after coalescing, leaving just the new column. Default false keeps them."))
        .param(Param::boolean("blank_is_empty").default(true).describe("Treat whitespace-only cells (a stray space or tab) as empty so they are skipped. Default true; false only skips truly zero-length cells."))
        .param(Param::string("null_tokens").default("").describe("Comma-separated placeholder values that also count as empty, matched case-insensitively on the trimmed cell, e.g. 'NULL,NA,N/A,-'. Default '' = only blank cells count as empty."))
        .param(Param::boolean("header").default(true).describe("Treat the first row as a header: it is rewritten with the new column and lets `columns` use names. Default true."))
        .param(Param::string("delimiter").default(",").describe("Field separator: a single char or 'comma'/'tab'/'semicolon'/'pipe'. Default ','."))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct CsvCoalesceColumns;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/csv-coalesce-columns",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Combine CSV columns into one, taking the first non-empty value per row",
    skill(
        description = "Coalesce several CSV columns into one: each row takes the first non-empty value across `columns`, read in the order you list them (SQL COALESCE over columns). `columns` accepts header names (header=true) or 1-based indices. `output` names the new column (default 'coalesced'), `position` places it (end/start/first-source), `fallback` fills rows where every source is empty, and `drop_sources`=true removes the sources so only the merged column remains. `blank_is_empty`=true (default) also skips whitespace-only cells, and `null_tokens` (e.g. 'NULL,NA,-') lists placeholder values that count as empty too. `delimiter` is a char or comma/tab/semicolon/pipe.",
        parameters = schema_json()
    ),
)]
impl CsvCoalesceColumns {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "csv-coalesce-columns", |a: Args| {
            let delim = if a.delimiter.is_empty() {
                ",".to_string()
            } else {
                a.delimiter
            };
            coalesce_columns(
                &a.data,
                &a.columns,
                &a.output,
                &a.position,
                &a.fallback,
                a.drop_sources,
                a.blank_is_empty,
                &a.null_tokens,
                a.header,
                &delim,
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
                    "data":           { "type": "string", "description": "The CSV text to process, e.g. 'name,mobile,office\\nAnn,,555-2'. Rows shorter than the header are padded with empty cells." },
                    "columns":        { "type": "string", "description": "Comma-separated source columns in PRIORITY order — the first one that has a value wins. Use header names when header=true (e.g. 'mobile,office,home') or 1-based indices (e.g. '2,3,4')." },
                    "output":         { "type": "string", "default": "", "description": "Name for the new coalesced column. Default '' = 'coalesced'. It must not collide with a column you keep." },
                    "position":       { "type": "string", "enum": ["end", "start", "first-source"], "default": "end", "description": "Where the new column goes: 'end' appends it after the last column (default), 'start' puts it first, 'first-source' puts it where the first listed source column sat." },
                    "fallback":       { "type": "string", "default": "", "description": "Value written when EVERY source column is empty for that row, e.g. 'N/A' or 'unknown'. Default '' leaves the cell blank." },
                    "drop_sources":   { "type": "boolean", "default": false, "description": "Remove the source columns after coalescing, leaving just the new column. Default false keeps them." },
                    "blank_is_empty": { "type": "boolean", "default": true, "description": "Treat whitespace-only cells (a stray space or tab) as empty so they are skipped. Default true; false only skips truly zero-length cells." },
                    "null_tokens":    { "type": "string", "default": "", "description": "Comma-separated placeholder values that also count as empty, matched case-insensitively on the trimmed cell, e.g. 'NULL,NA,N/A,-'. Default '' = only blank cells count as empty." },
                    "header":         { "type": "boolean", "default": true, "description": "Treat the first row as a header: it is rewritten with the new column and lets `columns` use names. Default true." },
                    "delimiter":      { "type": "string", "default": ",", "description": "Field separator: a single char or 'comma'/'tab'/'semicolon'/'pipe'. Default ','." }
                },
                "required": ["data", "columns"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
