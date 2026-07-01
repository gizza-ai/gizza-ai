//! gizza-ai/parse-websocket-frame — chat skill block on the shared tool abstraction.
//! Decodes a single WebSocket frame (RFC 6455) into its FIN/RSV/opcode, mask flag,
//! payload length, and the unmasked payload bytes. The chat schema is single-sourced
//! from descriptor() (which also drives the CLI); handle() delegates to
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
                .describe("The WebSocket frame, as a base64 or hex byte string (e.g. the bytes captured off the wire). Only the first frame is decoded."),
        )
        .param(
            Param::enumv("encoding", ["auto", "base64", "hex"])
                .default("auto")
                .describe("How the input bytes are encoded. 'auto' (default) detects hex (only hex digits, even length) else base64. base64 accepts standard or URL-safe, padding optional; hex ignores spaces, ':' and '-'."),
        )
        .param(
            Param::enumv("format", ["json", "text"])
                .default("json")
                .describe("Output shape. 'json' (default) is a structured object {fin, rsv1/2/3, opcode, opcode_name, masked, payload_length, masking_key, payload_hex, payload_text?, close?}; 'text' is a compact human-readable summary."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ParseWebsocketFrame;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/parse-websocket-frame",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Decode a WebSocket frame (RFC 6455) into FIN/RSV/opcode, mask flag, payload length, and the unmasked payload bytes.",
    skill(
        description = "Decode a WebSocket frame (RFC 6455) into its FIN bit, RSV1/RSV2/RSV3 reserved bits, opcode (numeric + human name: continuation/text/binary/close/ping/pong), the MASK flag, the payload length, the 4-byte masking key (when present), and the unmasked payload bytes. Give the frame bytes as base64 or hex in 'input' (encoding='auto' (default), 'base64', or 'hex'). byte0 carries FIN (0x80) + RSV (0x40/0x20/0x10) + opcode (low nibble); byte1 carries MASK (0x80) + a 7-bit length where 126 means the real length is the next 2 bytes (big-endian) and 127 means the next 8 bytes; a masked frame then has a 4-byte key and the payload is XOR-unmasked with key[i%4]. The output reports payload_hex always, payload_text (UTF-8) for text frames, and a {code, reason} object for close frames. Use format='json' (default) for a structured object or 'text' for a compact summary. Truncated/too-short frames return a clear error. Useful for debugging a WebSocket handshake or a captured ws:// byte dump.",
        parameters = schema_json()
    )
)]
impl ParseWebsocketFrame {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "parse-websocket-frame", |a: Args| {
            gizza_ai_parse_websocket_frame_core::parse(&a.input, &a.encoding, &a.format)
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
                    "input": { "type": "string", "description": "The WebSocket frame, as a base64 or hex byte string (e.g. the bytes captured off the wire). Only the first frame is decoded." },
                    "encoding": { "type": "string", "enum": ["auto", "base64", "hex"], "default": "auto", "description": "How the input bytes are encoded. 'auto' (default) detects hex (only hex digits, even length) else base64. base64 accepts standard or URL-safe, padding optional; hex ignores spaces, ':' and '-'." },
                    "format": { "type": "string", "enum": ["json", "text"], "default": "json", "description": "Output shape. 'json' (default) is a structured object {fin, rsv1/2/3, opcode, opcode_name, masked, payload_length, masking_key, payload_hex, payload_text?, close?}; 'text' is a compact human-readable summary." }
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
