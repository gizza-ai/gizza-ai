//! gizza-ai/csv-cleaner — clean a CSV in one pass (trim, dedupe, drop empty rows,
//! fill empty cells, normalize delimiter + line endings). Thin wrapper around the
//! core; the chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. Pure.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_csv_cleaner_core::clean;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default = "default_true")]
    header: bool,
    #[serde(default)]
    delimiter: String,
    #[serde(default)]
    output_delimiter: String,
    #[serde(default = "default_true")]
    trim: bool,
    #[serde(default = "default_true")]
    dedupe: bool,
    #[serde(default = "default_true")]
    drop_empty_rows: bool,
    #[serde(default)]
    empty_cells: String,
    #[serde(default)]
    fill_value: String,
    #[serde(default)]
    line_ending: String,
}
fn default_true() -> bool { true }

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe("The CSV text to clean."))
        .param(Param::boolean("header").default(true).describe("Treat the first row as a header: keep it and exempt it from dedup, empty-row drop, and cell fill. Default true."))
        .param(Param::string("delimiter").default(",").describe("Input field separator: a single char or 'comma'/'tab'/'semicolon'/'pipe'. Default ','."))
        .param(Param::enumv("output_delimiter", ["same", "comma", "tab", "semicolon", "pipe"]).default("same").describe("Delimiter for the cleaned output. 'same' keeps the input delimiter. Default 'same'."))
        .param(Param::boolean("trim").default(true).describe("Strip leading/trailing whitespace from every cell. Default true."))
        .param(Param::boolean("dedupe").default(true).describe("Remove rows identical to an earlier row (first kept). Default true."))
        .param(Param::boolean("drop_empty_rows").default(true).describe("Drop rows whose every cell is empty or whitespace-only. Default true."))
        .param(Param::enumv("empty_cells", ["keep", "fill"]).default("keep").describe("Empty cells: 'keep' leaves them, 'fill' replaces them with fill_value. Default 'keep'."))
        .param(Param::string("fill_value").default("").describe("Value written into empty cells when empty_cells='fill' (e.g. 'N/A' or '0'). Default empty."))
        .param(Param::enumv("line_ending", ["lf", "crlf"]).default("lf").describe("Line ending for the output: 'lf' (\\n) or 'crlf' (\\r\\n, Windows/Excel). Default 'lf'."))
}
fn schema_json() -> String { descriptor().to_schema_json() }

#[cfg(target_arch = "wasm32")]
struct CsvCleaner;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/csv-cleaner",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Clean a CSV: trim, dedupe, drop empty rows, fill cells, normalize delimiter",
    skill(
        description = "Clean a CSV in one pass: trim leading/trailing whitespace from every cell, remove duplicate rows (first kept), drop fully-empty rows, optionally fill empty cells with a value, and normalize the delimiter and line endings. `header`=true keeps the first row and exempts it from dedup/drop/fill. Each cleaning step is an independent toggle. Delimiters accept a char or comma/tab/semicolon/pipe.",
        parameters = schema_json()
    ),
)]
impl CsvCleaner {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "csv-cleaner", |a: Args| {
            let delimiter = if a.delimiter.is_empty() { ",".to_string() } else { a.delimiter };
            let output_delimiter = if a.output_delimiter.is_empty() { "same".to_string() } else { a.output_delimiter };
            let empty_cells = if a.empty_cells.is_empty() { "keep".to_string() } else { a.empty_cells };
            let line_ending = if a.line_ending.is_empty() { "lf".to_string() } else { a.line_ending };
            clean(
                &a.data,
                a.header,
                &delimiter,
                &output_delimiter,
                a.trim,
                a.dedupe,
                a.drop_empty_rows,
                &empty_cells,
                &a.fill_value,
                &line_ending,
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
                    "data":             { "type": "string", "description": "The CSV text to clean." },
                    "header":           { "type": "boolean", "default": true, "description": "Treat the first row as a header: keep it and exempt it from dedup, empty-row drop, and cell fill. Default true." },
                    "delimiter":        { "type": "string", "default": ",", "description": "Input field separator: a single char or 'comma'/'tab'/'semicolon'/'pipe'. Default ','." },
                    "output_delimiter": { "type": "string", "enum": ["same", "comma", "tab", "semicolon", "pipe"], "default": "same", "description": "Delimiter for the cleaned output. 'same' keeps the input delimiter. Default 'same'." },
                    "trim":             { "type": "boolean", "default": true, "description": "Strip leading/trailing whitespace from every cell. Default true." },
                    "dedupe":           { "type": "boolean", "default": true, "description": "Remove rows identical to an earlier row (first kept). Default true." },
                    "drop_empty_rows":  { "type": "boolean", "default": true, "description": "Drop rows whose every cell is empty or whitespace-only. Default true." },
                    "empty_cells":      { "type": "string", "enum": ["keep", "fill"], "default": "keep", "description": "Empty cells: 'keep' leaves them, 'fill' replaces them with fill_value. Default 'keep'." },
                    "fill_value":       { "type": "string", "default": "", "description": "Value written into empty cells when empty_cells='fill' (e.g. 'N/A' or '0'). Default empty." },
                    "line_ending":      { "type": "string", "enum": ["lf", "crlf"], "default": "lf", "description": "Line ending for the output: 'lf' (\\n) or 'crlf' (\\r\\n, Windows/Excel). Default 'lf'." }
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
