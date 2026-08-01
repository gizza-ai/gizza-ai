//! gizza-ai/ndjson-filter — filter + reshape newline-delimited JSON (NDJSON /
//! JSON Lines) records with a small deterministic predicate language and a
//! field-selection expression.
//!
//! Thin chat-skill wrapper around `gizza-ai-ndjson-filter-core`. The chat schema
//! is derived from `descriptor()` (single source — shared shape across chat +
//! CLI); the handler delegates to `block_utils::run_skill`. No host calls — runs
//! entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default)]
    predicate: String,
    #[serde(default)]
    fields: String,
    #[serde(default)]
    format: String,
    #[serde(default)]
    invert: bool,
    /// Max rows to keep; 0 (default) = unlimited.
    #[serde(default)]
    limit: u32,
    #[serde(default)]
    skip_invalid: bool,
}

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("data")
                .required()
                .describe("The NDJSON / JSON Lines input: one JSON value (usually an object) per line. Blank lines are skipped. Example: {\"name\":\"Alice\",\"age\":30}\\n{\"name\":\"Bob\",\"age\":25}"),
        )
        .param(
            Param::string("predicate")
                .default("")
                .describe("Filter expression evaluated per record; empty keeps every row. A clause is 'path op value' (e.g. age > 28, city == NYC, email contains @x, name ~ ^A). Ops: == != > >= < <= contains startswith endswith ~/matches. Join clauses with and / or / not and group with ( ). A bare path (e.g. active) is true when that field exists and is truthy. Dotted paths (user.name) and array indexes (items.0.id) are supported."),
        )
        .param(
            Param::string("fields")
                .default("")
                .describe("Comma-separated field selection / reshape; empty keeps whole records. Each entry is 'path' or 'newname=path', e.g. name,age or id=user.id,email. Dotted paths index nested objects/arrays; a missing path yields null."),
        )
        .param(
            Param::enumv("format", ["ndjson", "array", "csv"])
                .default("ndjson")
                .describe("Output format. 'ndjson' (default): one compact JSON object per line. 'array': a pretty-printed JSON array. 'csv': RFC-4180 CSV whose columns are the first-seen union of the kept records' keys."),
        )
        .param(
            Param::boolean("invert")
                .default(false)
                .describe("When true, keep the records that do NOT match the predicate (the complement). Default false."),
        )
        .param(
            Param::integer("limit")
                .default(0)
                .min(0.0)
                .describe("Maximum number of records to output; 0 (default) = unlimited. Filtering stops once this many matching rows are kept."),
        )
        .param(
            Param::boolean("skip_invalid")
                .default(false)
                .describe("When true, silently drop lines that are not valid JSON. Default false, which errors on the first malformed line with its line number."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct NdjsonFilter;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/ndjson-filter",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Filter and reshape NDJSON / JSON Lines with a predicate and field selection.",
    skill(
        description = "Filter and reshape newline-delimited JSON (NDJSON / JSON Lines): one JSON object per line in, filtered/reshaped records out. 'predicate' is a per-record expression of 'path op value' clauses (ops == != > >= < <= contains startswith endswith ~/matches) joined by and / or / not with ( ) grouping; a bare path means exists-and-truthy; dotted paths and array indexes work. 'fields' selects/renames columns (comma-separated 'newname=path'). 'format' is ndjson (default), array, or csv. 'invert' keeps non-matching rows, 'limit' caps the count (0 = all), and 'skip_invalid' drops malformed lines instead of erroring. Returns a line-numbered error on invalid JSON when skip_invalid is off.",
        parameters = schema_json()
    ),
)]
impl NdjsonFilter {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "ndjson-filter", |a: Args| {
            gizza_ai_ndjson_filter_core::filter(
                &a.data,
                &a.predicate,
                &a.fields,
                &a.format,
                a.invert,
                a.limit as usize,
                a.skip_invalid,
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
                    "data": { "type": "string", "description": "The NDJSON / JSON Lines input: one JSON value (usually an object) per line. Blank lines are skipped. Example: {\"name\":\"Alice\",\"age\":30}\\n{\"name\":\"Bob\",\"age\":25}" },
                    "predicate": { "type": "string", "default": "", "description": "Filter expression evaluated per record; empty keeps every row. A clause is 'path op value' (e.g. age > 28, city == NYC, email contains @x, name ~ ^A). Ops: == != > >= < <= contains startswith endswith ~/matches. Join clauses with and / or / not and group with ( ). A bare path (e.g. active) is true when that field exists and is truthy. Dotted paths (user.name) and array indexes (items.0.id) are supported." },
                    "fields": { "type": "string", "default": "", "description": "Comma-separated field selection / reshape; empty keeps whole records. Each entry is 'path' or 'newname=path', e.g. name,age or id=user.id,email. Dotted paths index nested objects/arrays; a missing path yields null." },
                    "format": { "type": "string", "enum": ["ndjson", "array", "csv"], "default": "ndjson", "description": "Output format. 'ndjson' (default): one compact JSON object per line. 'array': a pretty-printed JSON array. 'csv': RFC-4180 CSV whose columns are the first-seen union of the kept records' keys." },
                    "invert": { "type": "boolean", "default": false, "description": "When true, keep the records that do NOT match the predicate (the complement). Default false." },
                    "limit": { "type": "integer", "minimum": 0, "default": 0, "description": "Maximum number of records to output; 0 (default) = unlimited. Filtering stops once this many matching rows are kept." },
                    "skip_invalid": { "type": "boolean", "default": false, "description": "When true, silently drop lines that are not valid JSON. Default false, which errors on the first malformed line with its line number." }
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
