//! gizza-ai/protobuf-decode — decode raw Protocol Buffers wire bytes into a
//! field-number / wire-type tree, without a `.proto` schema.
//!
//! Thin chat-skill wrapper around `gizza-ai-protobuf-decode-core`. The chat
//! schema is single-sourced from `descriptor()` (shared shape across chat +
//! CLI); the handler delegates to `block_utils::run_skill`. No host calls —
//! runs entirely inside the WASM sandbox.
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
                .describe("The raw protobuf wire bytes, as a base64 or hex string."),
        )
        .param(
            Param::enumv("encoding", ["auto", "base64", "hex"])
                .default("auto")
                .describe("How the input bytes are encoded. 'auto' (default) detects hex (only hex digits, even length) else base64. base64 accepts standard or URL-safe, padding optional; hex ignores spaces, ':' and '-'."),
        )
        .param(
            Param::enumv("format", ["json", "text"])
                .default("json")
                .describe("Output shape. 'json' (default) is a structured tree of {field, wire_type, ...interpretations}; 'text' is a compact indented outline."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ProtobufDecode;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/protobuf-decode",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Decode raw protobuf wire bytes into a field/wire-type tree without a .proto schema.",
    skill(
        description = "Decode raw Protocol Buffers (protobuf) wire bytes into a field-number / wire-type tree WITHOUT a .proto schema. Give the wire bytes as base64 or hex in 'input' (set encoding='auto' (default), 'base64', or 'hex'). Each field is reported by its field number, raw wire type (varint / fixed32 / fixed64 / length_delimited) and every plausible interpretation, since the schemaless wire format is ambiguous: a varint is shown as uint/int/zigzag (and bool when 0/1); fixed32 as uint32/int32/float; fixed64 as uint64/int64/double; a length-delimited field is recursively parsed as a nested message when it parses cleanly, and also shown as a string (when valid text) and as hex bytes. Use format='json' (default) for a structured tree or 'text' for a compact outline. Useful for reverse-engineering an undocumented gRPC / protobuf payload.",
        parameters = schema_json()
    )
)]
impl ProtobufDecode {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "protobuf-decode", |a: Args| {
            gizza_ai_protobuf_decode_core::decode(&a.input, &a.encoding, &a.format)
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
                    "input": { "type": "string", "description": "The raw protobuf wire bytes, as a base64 or hex string." },
                    "encoding": { "type": "string", "enum": ["auto", "base64", "hex"], "default": "auto", "description": "How the input bytes are encoded. 'auto' (default) detects hex (only hex digits, even length) else base64. base64 accepts standard or URL-safe, padding optional; hex ignores spaces, ':' and '-'." },
                    "format": { "type": "string", "enum": ["json", "text"], "default": "json", "description": "Output shape. 'json' (default) is a structured tree of {field, wire_type, ...interpretations}; 'text' is a compact indented outline." }
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
