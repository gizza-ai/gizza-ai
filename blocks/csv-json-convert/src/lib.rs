//! gizza-ai/csv-json-convert — converts tabular data between CSV and JSON.
//!
//! Thin chat-skill wrapper around `gizza-ai-csv-json-convert-core`. The chat
//! schema is derived from `descriptor()` (single source — shared across chat +
//! CLI + page query-params); the handler delegates to `block_utils::run_skill`.
//! No host calls — runs entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_csv_json_convert_core::{convert, Direction};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default)]
    direction: String,
    #[serde(default)]
    delimiter: String,
    #[serde(default = "default_true")]
    headers: bool,
    #[serde(default)]
    pretty: bool,
    #[serde(default)]
    flatten: bool,
}

fn default_true() -> bool {
    true
}

/// Single-source param descriptor → chat schema (and CLI + page query-params).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("data")
                .required()
                .describe("The CSV or JSON text to convert."),
        )
        .param(
            Param::enumv("direction", ["auto", "csv-to-json", "json-to-csv"])
                .default("auto")
                .describe("Conversion direction. 'auto' (default) detects the input: text starting with '[' or '{' is treated as JSON (-> CSV), otherwise CSV (-> JSON)."),
        )
        .param(
            Param::string("delimiter")
                .default(",")
                .describe("CSV field delimiter -- a single character, or the word 'tab', 'comma', 'semicolon', or 'pipe'. Default ','."),
        )
        .param(
            Param::boolean("headers")
                .default(true)
                .describe("Treat the CSV's first row as field names. csv->json: true gives an array of objects, false an array of arrays. json->csv: true emits a header row from the object keys. Default true."),
        )
        .param(
            Param::boolean("pretty")
                .default(false)
                .describe("Pretty-print (indent) JSON output. Only affects csv->json. Default false."),
        )
        .param(
            Param::boolean("flatten")
                .default(false)
                .describe("json->csv only: expand nested objects/arrays into dot-notation columns (e.g. {\"addr\":{\"city\":\"NYC\"}} becomes column 'addr.city'). When false, nested values are written as compact JSON strings. Default false."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct CsvJsonConvert;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/csv-json-convert",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "CSV <-> JSON convert skill",
    skill(
        description = "Convert tabular data between CSV and JSON in either direction. direction='auto' (default) detects the input format (text starting with '[' or '{' is JSON -> CSV, otherwise CSV -> JSON); set 'csv-to-json' or 'json-to-csv' to force it. csv->json yields an array of objects keyed by the header row (headers=true, default) or an array of arrays (headers=false), inferring numbers/booleans/null from cell text. json->csv accepts an array of objects (header row from the union of keys) or an array of arrays. delimiter is a single char or 'tab'/'comma'/'semicolon'/'pipe'. pretty=true indents JSON output. flatten=true (json->csv) expands nested objects/arrays into dot-notation columns.",
        parameters = schema_json()
    )
)]
impl CsvJsonConvert {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "csv-json-convert", |a: Args| {
            let dir = Direction::parse(&a.direction).map_err(SkillError::InvalidArgs)?;
            convert(&a.data, dir, &a.delimiter, a.headers, a.pretty, a.flatten)
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
    /// reviewed. (Regenerate this literal when the descriptor changes -- see
    /// /improve-tool reference.md "drift-guard".)
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "data": { "type": "string", "description": "The CSV or JSON text to convert." },
                    "direction": { "type": "string", "enum": ["auto", "csv-to-json", "json-to-csv"], "default": "auto", "description": "Conversion direction. 'auto' (default) detects the input: text starting with '[' or '{' is treated as JSON (-> CSV), otherwise CSV (-> JSON)." },
                    "delimiter": { "type": "string", "default": ",", "description": "CSV field delimiter -- a single character, or the word 'tab', 'comma', 'semicolon', or 'pipe'. Default ','." },
                    "headers": { "type": "boolean", "default": true, "description": "Treat the CSV's first row as field names. csv->json: true gives an array of objects, false an array of arrays. json->csv: true emits a header row from the object keys. Default true." },
                    "pretty": { "type": "boolean", "default": false, "description": "Pretty-print (indent) JSON output. Only affects csv->json. Default false." },
                    "flatten": { "type": "boolean", "default": false, "description": "json->csv only: expand nested objects/arrays into dot-notation columns (e.g. {\"addr\":{\"city\":\"NYC\"}} becomes column 'addr.city'). When false, nested values are written as compact JSON strings. Default false." }
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
