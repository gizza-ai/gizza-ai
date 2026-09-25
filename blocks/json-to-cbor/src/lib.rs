//! gizza-ai/json-to-cbor — encode JSON into RFC 8949 CBOR bytes.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_json_to_cbor_core::{run_with_options, Options};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    json: String,
    #[serde(default = "default_output")]
    output: String,
    #[serde(default = "default_true")]
    canonical: bool,
    #[serde(default)]
    group: u32,
}
fn default_output() -> String {
    "hex".to_string()
}
fn default_true() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("json")
                .required()
                .describe("JSON value to encode as CBOR: object, array, string, number, boolean, or null."),
        )
        .param(
            Param::enumv("output", ["hex", "base64", "summary", "json"])
                .default("hex")
                .describe("Output format: raw lowercase hex, Base64, a readable summary with size comparison, or a JSON wrapper containing both encodings."),
        )
        .param(
            Param::boolean("canonical")
                .default(true)
                .describe("Sort object keys by canonical CBOR key bytes for reproducible RFC 8949-style output (default true)."),
        )
        .param(
            Param::integer("group")
                .default(0)
                .min(0.0)
                .max(32.0)
                .describe("For hex output, insert spaces every N bytes for readability. Use 0 for continuous hex (default)."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/json-to-cbor",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Encode JSON as CBOR bytes in hex or Base64",
    skill(
        description = "Encode a pasted JSON value into RFC 8949 CBOR bytes locally. Supports JSON null, booleans, integers, floating-point numbers, strings, arrays, and objects. Output can be lowercase hex, Base64, a readable summary with JSON-vs-CBOR byte counts, or a JSON wrapper containing both encodings. canonical=true sorts object keys by their encoded CBOR key bytes for reproducible output; group inserts spaces in hex output every N bytes.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "json-to-cbor", |a: Args| {
            run_with_options(
                &a.json,
                &Options {
                    output: a.output,
                    canonical: a.canonical,
                    group: a.group,
                },
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
                    "json": { "type": "string", "description": "JSON value to encode as CBOR: object, array, string, number, boolean, or null." },
                    "output": { "type": "string", "enum": ["hex", "base64", "summary", "json"], "default": "hex", "description": "Output format: raw lowercase hex, Base64, a readable summary with size comparison, or a JSON wrapper containing both encodings." },
                    "canonical": { "type": "boolean", "default": true, "description": "Sort object keys by canonical CBOR key bytes for reproducible RFC 8949-style output (default true)." },
                    "group": { "type": "integer", "default": 0, "minimum": 0, "maximum": 32, "description": "For hex output, insert spaces every N bytes for readability. Use 0 for continuous hex (default)." }
                },
                "required": ["json"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
