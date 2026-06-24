//! gizza-ai/avro-to-json — decode an Apache Avro Object Container File (OCF) into
//! readable JSON records. The OCF embeds its writer schema, so no `.avsc` is
//! needed. Thin chat-skill wrapper around `gizza-ai-avro-to-json-core`; the chat
//! schema is single-sourced from `descriptor()` (shared across chat + CLI); the
//! handler delegates to `block_utils::run_skill`. Pure Rust → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default)]
    encoding: String,
    #[serde(default)]
    format: String,
}

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("The Avro Object Container File (.avro) bytes, as a base64 or hex string."),
        )
        .param(
            Param::enumv("encoding", ["auto", "base64", "hex"])
                .default("auto")
                .describe("How the input bytes are encoded. 'auto' (default) detects hex (only hex digits, even length) else base64. base64 accepts standard or URL-safe, padding optional; hex ignores spaces, ':' and '-'."),
        )
        .param(
            Param::enumv("format", ["records", "ndjson", "full"])
                .default("records")
                .describe("Output shape. 'records' (default) is a pretty JSON array of the records; 'ndjson' is one compact JSON record per line; 'full' is { schema, count, records } including the embedded writer schema."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct AvroToJson;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/avro-to-json",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Decode an Avro OCF into JSON records",
    skill(
        description = "Decode an Apache Avro Object Container File (.avro / OCF) into readable JSON records. The OCF embeds its own writer schema, so NO external .avsc is needed. Give the file bytes as base64 or hex in 'input' (set encoding='auto' (default), 'base64', or 'hex'). Use format='records' (default) for a pretty JSON array, 'ndjson' for one JSON record per line, or 'full' for { schema, count, records } including the embedded writer schema. Logical types are decoded (dates/timestamps as integers, uuid as a string, decimals/bytes/fixed as base64). Runs locally — the data never leaves the device. Note: this reads OCF container files, not bare/single-object-encoded Avro (which carry no schema).",
        parameters = schema_json()
    ),
)]
impl AvroToJson {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "avro-to-json", |a: Args| {
            gizza_ai_avro_to_json_core::decode(&a.input, &a.encoding, &a.format)
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
    /// schema, so any future change to the LLM-facing API is intentional.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "input": { "type": "string", "description": "The Avro Object Container File (.avro) bytes, as a base64 or hex string." },
                    "encoding": { "type": "string", "enum": ["auto", "base64", "hex"], "default": "auto", "description": "How the input bytes are encoded. 'auto' (default) detects hex (only hex digits, even length) else base64. base64 accepts standard or URL-safe, padding optional; hex ignores spaces, ':' and '-'." },
                    "format": { "type": "string", "enum": ["records", "ndjson", "full"], "default": "records", "description": "Output shape. 'records' (default) is a pretty JSON array of the records; 'ndjson' is one compact JSON record per line; 'full' is { schema, count, records } including the embedded writer schema." }
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
