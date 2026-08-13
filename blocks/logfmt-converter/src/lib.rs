//! gizza-ai/logfmt-converter — converts log/event data between logfmt, JSON,
//! NDJSON (JSONL) and CSV in any direction.
//!
//! Thin chat-skill wrapper around `gizza-ai-logfmt-converter-core`. The chat
//! schema is derived from `descriptor()` (single source — shared across chat +
//! CLI + page query-params); the handler delegates to `block_utils::run_skill`.
//! No host calls — runs entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_logfmt_converter_core::convert;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default = "default_from")]
    from: String,
    #[serde(default = "default_to")]
    to: String,
    #[serde(default = "default_delimiter")]
    delimiter: String,
    #[serde(default = "default_true")]
    detect_types: bool,
    #[serde(default)]
    pretty: bool,
    #[serde(default = "default_true")]
    flatten: bool,
    #[serde(default)]
    keys: String,
}

fn default_true() -> bool {
    true
}

/// Mirror the descriptor defaults on the chat/CLI path, where an omitted param
/// deserializes via serde (the descriptor `.default(...)` only pre-fills the page
/// form and documents the schema — it is not injected into the invoke body).
fn default_from() -> String {
    "auto".to_string()
}

fn default_to() -> String {
    "json".to_string()
}

fn default_delimiter() -> String {
    "comma".to_string()
}

/// Single-source param descriptor → chat schema (and CLI + page query-params).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("data")
                .required()
                .describe("The logfmt, JSON, NDJSON, or CSV text to convert. One record per line for logfmt and NDJSON; blank lines are ignored. Maximum 1,000,000 characters."),
        )
        .param(
            Param::enumv("from", ["auto", "logfmt", "json", "ndjson", "csv"])
                .default("auto")
                .describe("Source format. 'auto' (default) detects it: text starting with '[' is a JSON array, '{' is a single JSON object (or 'ndjson' when every line is its own JSON value), a leading key=value token means 'logfmt', otherwise 'csv'. Set it explicitly when detection guesses wrong."),
        )
        .param(
            Param::enumv("to", ["logfmt", "json", "ndjson", "csv"])
                .default("json")
                .describe("Target format: 'logfmt' (key=value pairs, one record per line), 'json' (a single array), 'ndjson' (one compact JSON record per line, a.k.a. JSONL), or 'csv'. Default 'json'."),
        )
        .param(
            Param::enumv("delimiter", ["comma", "semicolon", "tab", "pipe"])
                .default("comma")
                .describe("Field separator used for CSV on BOTH sides — reading a CSV source and writing a CSV target. Ignored for the logfmt/JSON/NDJSON formats. Default 'comma'."),
        )
        .param(
            Param::boolean("detect_types")
                .default(true)
                .describe("For logfmt and CSV input: turn unquoted values into JSON numbers, true/false, and null (an empty logfmt value). Quoted values always stay strings, so msg=\"200\" survives as text. Leading-zero / '+'-prefixed values (zip codes, phone numbers) also stay strings. Ignored for JSON/NDJSON input. Default true."),
        )
        .param(
            Param::boolean("pretty")
                .default(false)
                .describe("Pretty-print (indent) JSON output. Only affects the 'json' target -- NDJSON is always one compact record per line. Default false."),
        )
        .param(
            Param::boolean("flatten")
                .default(true)
                .describe("When the target is flat (logfmt or CSV): expand nested objects/arrays into dot-notation keys (e.g. {\"user\":{\"id\":7}} becomes user.id=7, and a list becomes tags.0, tags.1). When false, nested values are written as compact JSON strings. Default true."),
        )
        .param(
            Param::string("keys")
                .default("")
                .describe("Optional comma-separated field allow-list that also fixes the output order, e.g. 'ts,level,msg'. Fields not listed are dropped; a record missing a listed field simply omits it. Blank (default) keeps every field in first-seen order."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct LogfmtConverter;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/logfmt-converter",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert log data between logfmt, JSON, NDJSON, and CSV in any direction.",
    skill(
        description = "Convert log/event data between logfmt (key=value pairs), JSON (a single array), NDJSON (one JSON record per line, a.k.a. JSONL) and CSV in any direction — including WRITING logfmt, which the other log tools cannot do. from='auto' (default) detects the source format; set from/to explicitly to force it. detect_types=true (default) coerces unquoted logfmt/CSV values to numbers, booleans and null, while quoted values stay strings. logfmt output follows the go-logfmt rules: single-space-separated key=value pairs, values double-quoted whenever they hold a space, '=', a quote or a control character, with \\\" \\\\ \\n \\r \\t escapes; a null writes as a bare 'key=' and an empty string as 'key=\"\"'. flatten=true (default) expands nested records into dot-notation keys when writing logfmt/CSV. delimiter picks the CSV separator, pretty indents the 'json' target, and keys is a comma-separated allow-list that also fixes field order.",
        parameters = schema_json()
    ),
)]
impl LogfmtConverter {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "logfmt-converter", |a: Args| {
            convert(
                &a.data,
                &a.from,
                &a.to,
                &a.delimiter,
                a.detect_types,
                a.pretty,
                a.flatten,
                &a.keys,
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
                    "data": { "type": "string", "description": "The logfmt, JSON, NDJSON, or CSV text to convert. One record per line for logfmt and NDJSON; blank lines are ignored. Maximum 1,000,000 characters." },
                    "from": { "type": "string", "enum": ["auto", "logfmt", "json", "ndjson", "csv"], "default": "auto", "description": "Source format. 'auto' (default) detects it: text starting with '[' is a JSON array, '{' is a single JSON object (or 'ndjson' when every line is its own JSON value), a leading key=value token means 'logfmt', otherwise 'csv'. Set it explicitly when detection guesses wrong." },
                    "to": { "type": "string", "enum": ["logfmt", "json", "ndjson", "csv"], "default": "json", "description": "Target format: 'logfmt' (key=value pairs, one record per line), 'json' (a single array), 'ndjson' (one compact JSON record per line, a.k.a. JSONL), or 'csv'. Default 'json'." },
                    "delimiter": { "type": "string", "enum": ["comma", "semicolon", "tab", "pipe"], "default": "comma", "description": "Field separator used for CSV on BOTH sides — reading a CSV source and writing a CSV target. Ignored for the logfmt/JSON/NDJSON formats. Default 'comma'." },
                    "detect_types": { "type": "boolean", "default": true, "description": "For logfmt and CSV input: turn unquoted values into JSON numbers, true/false, and null (an empty logfmt value). Quoted values always stay strings, so msg=\"200\" survives as text. Leading-zero / '+'-prefixed values (zip codes, phone numbers) also stay strings. Ignored for JSON/NDJSON input. Default true." },
                    "pretty": { "type": "boolean", "default": false, "description": "Pretty-print (indent) JSON output. Only affects the 'json' target -- NDJSON is always one compact record per line. Default false." },
                    "flatten": { "type": "boolean", "default": true, "description": "When the target is flat (logfmt or CSV): expand nested objects/arrays into dot-notation keys (e.g. {\"user\":{\"id\":7}} becomes user.id=7, and a list becomes tags.0, tags.1). When false, nested values are written as compact JSON strings. Default true." },
                    "keys": { "type": "string", "default": "", "description": "Optional comma-separated field allow-list that also fixes the output order, e.g. 'ts,level,msg'. Fields not listed are dropped; a record missing a listed field simply omits it. Blank (default) keeps every field in first-seen order." }
                },
                "required": ["data"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    /// The serde defaults on `Args` must mirror the descriptor defaults, since an
    /// omitted chat/CLI param never gets the descriptor's value injected.
    #[test]
    fn serde_defaults_mirror_descriptor_defaults() {
        let a: Args = serde_json::from_str(r#"{"data":"level=info n=2"}"#).unwrap();
        assert_eq!(a.from, "auto");
        assert_eq!(a.to, "json");
        assert_eq!(a.delimiter, "comma");
        assert!(a.detect_types);
        assert!(!a.pretty);
        assert!(a.flatten);
        assert_eq!(a.keys, "");
        let out = convert(
            &a.data,
            &a.from,
            &a.to,
            &a.delimiter,
            a.detect_types,
            a.pretty,
            a.flatten,
            &a.keys,
        )
        .unwrap();
        assert_eq!(out, r#"[{"level":"info","n":2}]"#);
    }
}
