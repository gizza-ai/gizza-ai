//! gizza-ai/parse-grpc-frame — chat skill block on the shared tool abstraction.
//! Splits a gRPC length-prefixed message stream (1-byte compressed flag +
//! 4-byte big-endian length per frame) into frames and decodes each embedded
//! protobuf payload into a field tree. The chat schema is single-sourced from
//! descriptor() (which also drives the CLI); handle() delegates to
//! block_utils::run_skill. Runs entirely inside the WASM sandbox, no host calls.
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
                .describe("The gRPC length-prefixed message stream, as a base64 or hex string. May contain several concatenated frames (e.g. the raw HTTP/2 DATA payload of a gRPC call)."),
        )
        .param(
            Param::enumv("encoding", ["auto", "base64", "hex"])
                .default("auto")
                .describe("How the input bytes are encoded. 'auto' (default) detects hex (only hex digits, even length) else base64. base64 accepts standard or URL-safe, padding optional; hex ignores spaces, ':' and '-'."),
        )
        .param(
            Param::enumv("format", ["json", "text"])
                .default("json")
                .describe("Output shape. 'json' (default) is a structured tree {frame_count, total_bytes, frames:[{index, compressed, length, message}]}; 'text' is a compact indented outline."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ParseGrpcFrame;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/parse-grpc-frame",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "gRPC frame parser skill",
    skill(
        description = "Parse a gRPC length-prefixed message stream and decode each embedded protobuf payload into a field tree — no .proto schema needed. gRPC wraps every message as a 1-byte compressed flag (0=uncompressed, 1=compressed) + a 4-byte big-endian length + that many payload bytes, and several messages can be concatenated in one HTTP/2 DATA body. Give the bytes as base64 or hex in 'input' (encoding='auto' (default), 'base64', or 'hex'). For each frame the output reports its index, compressed flag, declared length, and — for uncompressed frames — the protobuf payload decoded by field number / wire type (varint / fixed32 / fixed64 / length_delimited), recursing into nested messages. Compressed frames are reported with a hint to decompress first; payloads that aren't valid protobuf report a decode_error but still surface the hex bytes. Use format='json' (default) for a structured tree or 'text' for a compact outline. Useful for reverse-engineering a captured gRPC call.",
        parameters = schema_json()
    )
)]
impl ParseGrpcFrame {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "parse-grpc-frame", |a: Args| {
            gizza_ai_parse_grpc_frame_core::parse(&a.input, &a.encoding, &a.format)
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
                    "input": { "type": "string", "description": "The gRPC length-prefixed message stream, as a base64 or hex string. May contain several concatenated frames (e.g. the raw HTTP/2 DATA payload of a gRPC call)." },
                    "encoding": { "type": "string", "enum": ["auto", "base64", "hex"], "default": "auto", "description": "How the input bytes are encoded. 'auto' (default) detects hex (only hex digits, even length) else base64. base64 accepts standard or URL-safe, padding optional; hex ignores spaces, ':' and '-'." },
                    "format": { "type": "string", "enum": ["json", "text"], "default": "json", "description": "Output shape. 'json' (default) is a structured tree {frame_count, total_bytes, frames:[{index, compressed, length, message}]}; 'text' is a compact indented outline." }
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
