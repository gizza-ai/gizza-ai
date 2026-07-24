//! gizza-ai/csv-to-ndjson — converts CSV text to NDJSON (newline-delimited JSON).
//!
//! Thin chat-skill wrapper around `gizza-ai-csv-to-ndjson-core`. The chat schema is
//! derived from `descriptor()` (single source — shared across chat + CLI + page
//! query-params); the handler delegates to `block_utils::run_skill`. No host calls —
//! runs entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_csv_to_ndjson_core::to_ndjson;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default)]
    delimiter: String,
    #[serde(default = "default_true")]
    headers: bool,
    #[serde(default)]
    parse_numbers: bool,
    #[serde(default)]
    parse_bools: bool,
    #[serde(default)]
    trim: bool,
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
                .describe("The CSV text to convert. The first row is the header row unless headers=false. Handles quoted fields with embedded commas, newlines, and \"\" escapes (RFC 4180)."),
        )
        .param(
            Param::string("delimiter")
                .default(",")
                .describe("CSV field delimiter -- a single character, or the word 'tab', 'comma', 'semicolon', or 'pipe'. Default ','."),
        )
        .param(
            Param::boolean("headers")
                .default(true)
                .describe("Treat the first row as field names, so each row becomes a JSON object. false emits each row as a JSON array instead. Default true."),
        )
        .param(
            Param::boolean("parse_numbers")
                .default(false)
                .describe("Coerce numeric-looking cells to JSON numbers (e.g. 36 -> 36). Values with leading zeros or a leading + (007, +1) stay strings. Default false (all values stay strings)."),
        )
        .param(
            Param::boolean("parse_bools")
                .default(false)
                .describe("Coerce the literals true/false/null (and empty cells) to JSON booleans/null. Default false."),
        )
        .param(
            Param::boolean("trim")
                .default(false)
                .describe("Strip leading/trailing whitespace from every cell before conversion. Default false."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct CsvToNdjson;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/csv-to-ndjson",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert CSV text to NDJSON (one JSON value per line) for streaming pipelines.",
    skill(
        description = "Convert CSV text to NDJSON (newline-delimited JSON): one JSON value per line, no enclosing array -- the shape streaming ingest pipelines (jq, BigQuery load, Elasticsearch bulk, log shippers) expect. With headers=true (default) each row becomes a JSON object keyed by the first (header) row; headers=false emits each row as a JSON array. Values stay JSON strings unless you opt into inference: parse_numbers coerces numeric cells to numbers (leading-zero / +-prefixed values stay strings), parse_bools coerces true/false/null (and empty cells) to JSON booleans/null. trim strips surrounding whitespace from each cell. delimiter is a single char or 'tab'/'comma'/'semicolon'/'pipe'. Quoted fields with embedded commas/newlines and \"\" escapes are handled per RFC 4180.",
        parameters = schema_json()
    )
)]
impl CsvToNdjson {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "csv-to-ndjson", |a: Args| {
            to_ndjson(
                &a.data,
                &a.delimiter,
                a.headers,
                a.parse_numbers,
                a.parse_bools,
                a.trim,
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
    /// reviewed. (Regenerate this literal when the descriptor changes -- see
    /// /improve-tool reference.md "drift-guard".)
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "data": { "type": "string", "description": "The CSV text to convert. The first row is the header row unless headers=false. Handles quoted fields with embedded commas, newlines, and \"\" escapes (RFC 4180)." },
                    "delimiter": { "type": "string", "default": ",", "description": "CSV field delimiter -- a single character, or the word 'tab', 'comma', 'semicolon', or 'pipe'. Default ','." },
                    "headers": { "type": "boolean", "default": true, "description": "Treat the first row as field names, so each row becomes a JSON object. false emits each row as a JSON array instead. Default true." },
                    "parse_numbers": { "type": "boolean", "default": false, "description": "Coerce numeric-looking cells to JSON numbers (e.g. 36 -> 36). Values with leading zeros or a leading + (007, +1) stay strings. Default false (all values stay strings)." },
                    "parse_bools": { "type": "boolean", "default": false, "description": "Coerce the literals true/false/null (and empty cells) to JSON booleans/null. Default false." },
                    "trim": { "type": "boolean", "default": false, "description": "Strip leading/trailing whitespace from every cell before conversion. Default false." }
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
