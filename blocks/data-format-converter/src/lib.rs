//! gizza-ai/data-format-converter — converts tabular / record data between CSV,
//! TSV, JSON and NDJSON (JSONL) in any direction.
//!
//! Thin chat-skill wrapper around `gizza-ai-data-format-converter-core`. The chat
//! schema is derived from `descriptor()` (single source — shared across chat +
//! CLI + page query-params); the handler delegates to `block_utils::run_skill`.
//! No host calls — runs entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_data_format_converter_core::{convert, Format};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default)]
    from: String,
    #[serde(default = "default_to")]
    to: String,
    #[serde(default = "default_true")]
    headers: bool,
    #[serde(default = "default_true")]
    infer_types: bool,
    #[serde(default)]
    pretty: bool,
    #[serde(default)]
    flatten: bool,
}

fn default_true() -> bool {
    true
}

/// Mirror the descriptor default for `to` on the chat/CLI path, where an omitted
/// param deserializes via serde (the descriptor `.default("json")` only pre-fills
/// the page form and documents the schema — it is not injected into the invoke
/// body). An empty `to` would otherwise parse to `auto`, which is not a valid
/// target.
fn default_to() -> String {
    "json".to_string()
}

/// Single-source param descriptor → chat schema (and CLI + page query-params).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("data")
                .required()
                .describe("The CSV, TSV, JSON, or NDJSON text to convert."),
        )
        .param(
            Param::enumv("from", ["auto", "csv", "tsv", "json", "ndjson"])
                .default("auto")
                .describe("Source format. 'auto' (default) detects it: text starting with '[' is a JSON array, '{' is a single JSON object (or NDJSON if every line is its own JSON value), otherwise CSV (or TSV when the first row is tab-separated)."),
        )
        .param(
            Param::enumv("to", ["csv", "tsv", "json", "ndjson"])
                .default("json")
                .describe("Target format: 'json' (a single array), 'ndjson' (one JSON record per line, a.k.a. JSONL), 'csv' (comma-separated), or 'tsv' (tab-separated). Default 'json'."),
        )
        .param(
            Param::boolean("headers")
                .default(true)
                .describe("For CSV/TSV: treat the first row as field names. Parsing gives an array of objects (true) or arrays (false); writing emits a header row from the record keys. Default true."),
        )
        .param(
            Param::boolean("infer_types")
                .default(true)
                .describe("For CSV/TSV input: infer numbers, true/false, and empty->null from cell text. When false every cell stays a string. Leading-zero / '+'-prefixed values (zip codes, phone numbers) always stay strings. Ignored for JSON/NDJSON input. Default true."),
        )
        .param(
            Param::boolean("pretty")
                .default(false)
                .describe("Pretty-print (indent) JSON output. Only affects the 'json' target -- NDJSON is always one compact record per line. Default false."),
        )
        .param(
            Param::boolean("flatten")
                .default(false)
                .describe("When the target is CSV/TSV: expand nested objects/arrays into dot-notation columns (e.g. {\"addr\":{\"city\":\"NYC\"}} becomes column 'addr.city'). When false, nested values are written as compact JSON strings. Default false."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct DataFormatConverter;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/data-format-converter",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert tabular/record data between CSV, TSV, JSON, and NDJSON in any direction.",
    skill(
        description = "Convert tabular/record data between CSV, TSV, JSON (a single array) and NDJSON (one JSON record per line, a.k.a. JSONL) in any direction. from='auto' (default) detects the source format; set from/to explicitly to force it. CSV/TSV parse the header row into object keys (headers=true, default) and infer numbers/booleans/null from cell text (infer_types=true, default; leading-zero values stay strings). JSON/NDJSON records are written back to a table by the union of their keys. pretty=true indents the 'json' target; flatten=true expands nested objects/arrays into dot-notation columns when writing CSV/TSV.",
        parameters = schema_json()
    ),
)]
impl DataFormatConverter {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "data-format-converter", |a: Args| {
            let from = Format::parse(&a.from).map_err(SkillError::InvalidArgs)?;
            let to = Format::parse(&a.to).map_err(SkillError::InvalidArgs)?;
            convert(&a.data, from, to, a.headers, a.infer_types, a.pretty, a.flatten)
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
                    "data": { "type": "string", "description": "The CSV, TSV, JSON, or NDJSON text to convert." },
                    "from": { "type": "string", "enum": ["auto", "csv", "tsv", "json", "ndjson"], "default": "auto", "description": "Source format. 'auto' (default) detects it: text starting with '[' is a JSON array, '{' is a single JSON object (or NDJSON if every line is its own JSON value), otherwise CSV (or TSV when the first row is tab-separated)." },
                    "to": { "type": "string", "enum": ["csv", "tsv", "json", "ndjson"], "default": "json", "description": "Target format: 'json' (a single array), 'ndjson' (one JSON record per line, a.k.a. JSONL), 'csv' (comma-separated), or 'tsv' (tab-separated). Default 'json'." },
                    "headers": { "type": "boolean", "default": true, "description": "For CSV/TSV: treat the first row as field names. Parsing gives an array of objects (true) or arrays (false); writing emits a header row from the record keys. Default true." },
                    "infer_types": { "type": "boolean", "default": true, "description": "For CSV/TSV input: infer numbers, true/false, and empty->null from cell text. When false every cell stays a string. Leading-zero / '+'-prefixed values (zip codes, phone numbers) always stay strings. Ignored for JSON/NDJSON input. Default true." },
                    "pretty": { "type": "boolean", "default": false, "description": "Pretty-print (indent) JSON output. Only affects the 'json' target -- NDJSON is always one compact record per line. Default false." },
                    "flatten": { "type": "boolean", "default": false, "description": "When the target is CSV/TSV: expand nested objects/arrays into dot-notation columns (e.g. {\"addr\":{\"city\":\"NYC\"}} becomes column 'addr.city'). When false, nested values are written as compact JSON strings. Default false." }
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
