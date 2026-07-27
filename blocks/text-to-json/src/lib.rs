//! gizza-ai/text-to-json — chat skill block on the shared tool abstraction.
//!
//! Parses pasted structured text — INI/conf, key=value config, logfmt, CSV, or
//! `/etc/passwd`-style colon-delimited text — into JSON. The format is
//! auto-detected by default (and reported with `output=report`) or pinned via
//! `format`. The chat schema is single-sourced from `descriptor()` (which also
//! drives the CLI + page); `handle()` delegates to `block_utils::run_skill`. No
//! host calls — runs entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default)]
    format: String,
    #[serde(default = "default_true")]
    detect_types: bool,
    #[serde(default = "default_true")]
    pretty: bool,
    #[serde(default)]
    output: String,
}

fn default_true() -> bool {
    true
}

/// Single source for the chat schema (and CLI + page). See
/// docs/superpowers/specs/2026-06-19-gizza-shared-tool-abstraction-design.md.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The structured text to parse. Supported shapes: INI/.conf ('[section]' headers + 'key = value'), key=value config / .env / .properties (one pair per line), logfmt ('level=info msg=\"hi\" code=200' — many pairs per line, one record per line), CSV/TSV (a header row then rows; delimiter auto-sniffed from , ; tab |), and /etc/passwd-style colon-delimited lines. Lines starting with '#' or ';' are comments."),
        )
        .param(
            Param::enumv("format", ["auto", "ini", "keyvalue", "logfmt", "csv", "passwd"])
                .default("auto")
                .describe("Input format. 'auto' (default) detects it and, with output='report', tells you which was chosen; pin one of 'ini', 'keyvalue', 'logfmt', 'csv', or 'passwd' to override the guess (e.g. force single-column CSV or a colon file that isn't 7-field passwd)."),
        )
        .param(
            Param::boolean("detect_types")
                .default(true)
                .describe("When true (default), coerce unquoted values to JSON types: 'true'/'false' → boolean, integers and decimals → number, an empty value → null. Quoted values always stay strings. Set false to keep every value a lossless string (e.g. to preserve leading zeros like '007')."),
        )
        .param(
            Param::boolean("pretty")
                .default(true)
                .describe("When true (default), pretty-print the JSON with 2-space indentation; false minifies to one line. Ignored for output='ndjson' (each record is always compact on its own line)."),
        )
        .param(
            Param::enumv("output", ["json", "ndjson", "report"])
                .default("json")
                .describe("Output shape. 'json' (default) is the parsed value (a nested object for INI/keyvalue, an array of objects for logfmt/CSV/passwd); 'ndjson' emits one compact JSON record per line (JSON-Lines); 'report' wraps the data in '{ detected_format, record_count, data }' so you can see what auto-detect chose."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct TextToJson;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/text-to-json",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Parse pasted INI, key=value, logfmt, CSV, or passwd-style text into JSON, with format auto-detection.",
    skill(
        description = "Parse pasted structured text into JSON. Auto-detects (or you pin via 'format') one of: INI/.conf ('[section]' → nested object), key=value config / .env / .properties (flat object), logfmt ('k=v k2=\"a b\"' → array of objects, one per line, with quoted values and bare boolean keys), CSV/TSV (header row → array of objects; the , ; tab | delimiter is auto-sniffed, RFC-4180 quotes honored), or /etc/passwd-style colon lines (→ array with named fields name/password/uid/gid/gecos/home/shell, or 4-field group). detect_types (default true) coerces booleans/numbers and empty→null; set false to keep strings. pretty (default true) toggles indentation. output='json' (default), 'ndjson' (JSON-Lines), or 'report' (adds detected_format + record_count). Runs entirely in the sandbox — nothing is uploaded.",
        parameters = schema_json()
    ),
)]
impl TextToJson {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned string in { "result": … }.
        match run_skill(&body, "text-to-json", |a: Args| {
            gizza_ai_text_to_json_core::convert(
                &a.text,
                &a.format,
                a.detect_types,
                a.pretty,
                &a.output,
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
                    "text": { "type": "string", "description": "The structured text to parse. Supported shapes: INI/.conf ('[section]' headers + 'key = value'), key=value config / .env / .properties (one pair per line), logfmt ('level=info msg=\"hi\" code=200' — many pairs per line, one record per line), CSV/TSV (a header row then rows; delimiter auto-sniffed from , ; tab |), and /etc/passwd-style colon-delimited lines. Lines starting with '#' or ';' are comments." },
                    "format": { "type": "string", "enum": ["auto", "ini", "keyvalue", "logfmt", "csv", "passwd"], "default": "auto", "description": "Input format. 'auto' (default) detects it and, with output='report', tells you which was chosen; pin one of 'ini', 'keyvalue', 'logfmt', 'csv', or 'passwd' to override the guess (e.g. force single-column CSV or a colon file that isn't 7-field passwd)." },
                    "detect_types": { "type": "boolean", "default": true, "description": "When true (default), coerce unquoted values to JSON types: 'true'/'false' → boolean, integers and decimals → number, an empty value → null. Quoted values always stay strings. Set false to keep every value a lossless string (e.g. to preserve leading zeros like '007')." },
                    "pretty": { "type": "boolean", "default": true, "description": "When true (default), pretty-print the JSON with 2-space indentation; false minifies to one line. Ignored for output='ndjson' (each record is always compact on its own line)." },
                    "output": { "type": "string", "enum": ["json", "ndjson", "report"], "default": "json", "description": "Output shape. 'json' (default) is the parsed value (a nested object for INI/keyvalue, an array of objects for logfmt/CSV/passwd); 'ndjson' emits one compact JSON record per line (JSON-Lines); 'report' wraps the data in '{ detected_format, record_count, data }' so you can see what auto-detect chose." }
                },
                "required": ["text"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
