//! gizza-ai/binary-codec — encodes text or bytes to a per-byte binary bit string
//! and decodes a binary string back to text. Thin chat-skill wrapper around
//! `gizza-ai-binary-codec-core`. The chat schema is derived from `descriptor()`
//! (single source — shared shape across chat + CLI); the handler delegates to
//! `block_utils::run_skill`. No host calls — runs entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default)]
    mode: String,
    #[serde(default)]
    format: String,
    #[serde(default)]
    delimiter: String,
    #[serde(default)]
    prefix: String,
}

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("The text to encode (read as UTF-8), or the binary bit string to decode."),
        )
        .param(
            Param::enumv("mode", ["encode", "decode"])
                .default("encode")
                .describe("Direction: 'encode' (default) turns text into a binary bit string, 'decode' turns binary back into text."),
        )
        .param(
            Param::enumv("format", ["text", "bytes"])
                .default("text")
                .describe("On decode, how to render the recovered bytes: 'text' (default) is UTF-8 and errors if the bytes aren't valid UTF-8; 'bytes' shows them as a plain lowercase hex byte string (use for binary data). Ignored on encode."),
        )
        .param(
            Param::enumv("delimiter", ["none", "space", "colon", "dash", "comma", "newline"])
                .default("space")
                .describe("Separator placed between bytes when encoding: 'space' (default), 'none', 'colon' (:), 'dash' (-), 'comma', or 'newline'. Decoding ignores any of these."),
        )
        .param(
            Param::enumv("prefix", ["none", "0b"])
                .default("none")
                .describe("Marker placed before each byte when encoding: 'none' (default) or '0b' (e.g. 0b01001000). Decoding strips 0b automatically."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct BinaryCodec;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/binary-codec",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Binary encode/decode",
    skill(
        description = "Encode text or bytes to a per-byte binary bit string (eight 0/1 bits per byte), or decode a binary string back to text. Use mode='encode' (default, e.g. 'Hi' -> '01001000 01101001') or mode='decode' (e.g. '01001000 01101001' -> 'Hi'). delimiter sets the separator between bytes when encoding ('space' default, 'none', 'colon', 'dash', 'comma', 'newline'); prefix adds '0b' before each byte. On decode, format='text' (default) renders UTF-8 (errors on non-UTF-8 bytes) and format='bytes' shows the raw bytes as a plain hex string. Decoding ignores whitespace, the common delimiters (: - ,), and the 0b prefix, so any encoded form round-trips back.",
        parameters = schema_json()
    ),
)]
impl BinaryCodec {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned value in { "result": ... }.
        match run_skill(&body, "binary-codec", |a: Args| {
            gizza_ai_binary_codec_core::convert(
                &a.input, &a.mode, &a.format, &a.delimiter, &a.prefix,
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
                    "input": { "type": "string", "description": "The text to encode (read as UTF-8), or the binary bit string to decode." },
                    "mode": { "type": "string", "enum": ["encode", "decode"], "default": "encode", "description": "Direction: 'encode' (default) turns text into a binary bit string, 'decode' turns binary back into text." },
                    "format": { "type": "string", "enum": ["text", "bytes"], "default": "text", "description": "On decode, how to render the recovered bytes: 'text' (default) is UTF-8 and errors if the bytes aren't valid UTF-8; 'bytes' shows them as a plain lowercase hex byte string (use for binary data). Ignored on encode." },
                    "delimiter": { "type": "string", "enum": ["none", "space", "colon", "dash", "comma", "newline"], "default": "space", "description": "Separator placed between bytes when encoding: 'space' (default), 'none', 'colon' (:), 'dash' (-), 'comma', or 'newline'. Decoding ignores any of these." },
                    "prefix": { "type": "string", "enum": ["none", "0b"], "default": "none", "description": "Marker placed before each byte when encoding: 'none' (default) or '0b' (e.g. 0b01001000). Decoding strips 0b automatically." }
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
