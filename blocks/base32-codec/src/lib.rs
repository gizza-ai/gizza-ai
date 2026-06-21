//! gizza-ai/base32-codec — encodes text or bytes to RFC 4648 Base32 (plus the
//! hex, Crockford, and z-base-32 variants) and decodes Base32 back.
//!
//! Thin chat-skill wrapper around `gizza-ai-base32-codec-core`. The chat schema
//! is derived from `descriptor()` (single source — shared shape across chat +
//! CLI); the handler delegates to `block_utils::run_skill`. No host calls — runs
//! entirely inside the WASM sandbox.
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
    variant: String,
    #[serde(default)]
    format: String,
    /// Emit a lowercase alphabet when encoding standard/hex (default false).
    #[serde(default)]
    lowercase: bool,
    /// Append `=` padding when encoding standard/hex (default true).
    #[serde(default = "default_true")]
    padding: bool,
}

fn default_true() -> bool {
    true
}

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("The data to encode (text or hex bytes), or the Base32 string to decode."),
        )
        .param(
            Param::enumv("mode", ["encode", "decode"])
                .default("encode")
                .describe("Direction: 'encode' (default) turns data into Base32, 'decode' reverses it."),
        )
        .param(
            Param::enumv("variant", ["standard", "hex", "crockford", "zbase32"])
                .default("standard")
                .describe("Base32 alphabet. 'standard' (default) is RFC 4648 (A-Z 2-7); 'hex' is RFC 4648 base32hex (0-9 A-V); 'crockford' excludes the ambiguous I L O U and is never padded; 'zbase32' is the human-oriented z-base-32 (never padded)."),
        )
        .param(
            Param::enumv("format", ["text", "hex"])
                .default("text")
                .describe("How to read the bytes when encoding, or render them when decoding. 'text' (default) is UTF-8; 'hex' is a hex byte string (e.g. '48 65 6c' or '0x48656c') — use 'hex' for binary data that isn't valid UTF-8."),
        )
        .param(
            Param::boolean("lowercase")
                .default(false)
                .describe("When true, emit a lowercase alphabet while encoding standard/hex. Ignored for crockford/zbase32 and on decode (decoding is case-insensitive). Default false."),
        )
        .param(
            Param::boolean("padding")
                .default(true)
                .describe("When true (default), append '=' padding while encoding standard/hex per RFC 4648. Crockford and z-base-32 are never padded. Decoding accepts padded or unpadded input either way."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Base32Codec;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/base32-codec",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Base32 encode/decode skill",
    skill(
        description = "Encode text or bytes to Base32, or decode a Base32 string back to the original data. Use mode='encode' (default) or mode='decode'. variant selects the alphabet: 'standard' (default) is RFC 4648 (A-Z 2-7, e.g. 'foobar' -> 'MZXW6YTBOI======'); 'hex' is RFC 4648 base32hex; 'crockford' (no I L O U, never padded); 'zbase32' (human-oriented, never padded). format='text' (default) treats the data as UTF-8; format='hex' reads/writes a hex byte string for binary data that isn't valid UTF-8. lowercase emits a lowercase standard/hex alphabet; padding (default true) adds '=' padding for standard/hex. Decoding is case-insensitive and accepts padded or unpadded input.",
        parameters = schema_json()
    ),
)]
impl Base32Codec {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned value in { "result": ... }.
        match run_skill(&body, "base32-codec", |a: Args| {
            gizza_ai_base32_codec_core::convert(
                &a.input, &a.mode, &a.variant, &a.format, a.lowercase, a.padding,
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
                    "input": { "type": "string", "description": "The data to encode (text or hex bytes), or the Base32 string to decode." },
                    "mode": { "type": "string", "enum": ["encode", "decode"], "default": "encode", "description": "Direction: 'encode' (default) turns data into Base32, 'decode' reverses it." },
                    "variant": { "type": "string", "enum": ["standard", "hex", "crockford", "zbase32"], "default": "standard", "description": "Base32 alphabet. 'standard' (default) is RFC 4648 (A-Z 2-7); 'hex' is RFC 4648 base32hex (0-9 A-V); 'crockford' excludes the ambiguous I L O U and is never padded; 'zbase32' is the human-oriented z-base-32 (never padded)." },
                    "format": { "type": "string", "enum": ["text", "hex"], "default": "text", "description": "How to read the bytes when encoding, or render them when decoding. 'text' (default) is UTF-8; 'hex' is a hex byte string (e.g. '48 65 6c' or '0x48656c') — use 'hex' for binary data that isn't valid UTF-8." },
                    "lowercase": { "type": "boolean", "default": false, "description": "When true, emit a lowercase alphabet while encoding standard/hex. Ignored for crockford/zbase32 and on decode (decoding is case-insensitive). Default false." },
                    "padding": { "type": "boolean", "default": true, "description": "When true (default), append '=' padding while encoding standard/hex per RFC 4648. Crockford and z-base-32 are never padded. Decoding accepts padded or unpadded input either way." }
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
