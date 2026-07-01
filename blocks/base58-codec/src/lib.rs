//! gizza-ai/base58-codec — encodes text or bytes to Base58 (the Bitcoin/IPFS
//! alphabet, plus the Ripple and Flickr variants) and decodes Base58 back to
//! the original data, preserving leading-zero bytes.
//!
//! Thin chat-skill wrapper around `gizza-ai-base58-codec-core`. The chat schema
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
}

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("The data to encode (text or hex bytes), or the Base58 string to decode."),
        )
        .param(
            Param::enumv("mode", ["encode", "decode"])
                .default("encode")
                .describe("Direction: 'encode' (default) turns data into Base58, 'decode' reverses it."),
        )
        .param(
            Param::enumv("variant", ["bitcoin", "ripple", "flickr"])
                .default("bitcoin")
                .describe("Base58 alphabet. 'bitcoin' (default) is the Satoshi/IPFS alphabet (also used by Monero); 'ripple' is the XRP Ledger alphabet; 'flickr' is Flickr's short-URL alphabet (lowercase before uppercase). All exclude the ambiguous 0 O I l and never pad."),
        )
        .param(
            Param::enumv("format", ["text", "hex"])
                .default("text")
                .describe("How to read the bytes when encoding, or render them when decoding. 'text' (default) is UTF-8; 'hex' is a hex byte string (e.g. '48 65 6c' or '0x48656c') — use 'hex' for binary data that isn't valid UTF-8."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Base58Codec;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/base58-codec",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Encode text or bytes to Base58 (Bitcoin/IPFS) and decode Base58 back.",
    skill(
        description = "Encode text or bytes to Base58, or decode a Base58 string back to the original data. Use mode='encode' (default) or mode='decode'. variant selects the alphabet: 'bitcoin' (default) is the Satoshi/IPFS alphabet (e.g. 'Hello World!' -> '2NEpo7TZRRrLZSi2U'); 'ripple' is the XRP Ledger alphabet; 'flickr' is Flickr's short-URL alphabet. Base58 omits the ambiguous characters 0 O I l, has no padding, and preserves leading-zero bytes (each becomes a leading '1'). format='text' (default) treats the data as UTF-8; format='hex' reads/writes a hex byte string for binary data that isn't valid UTF-8.",
        parameters = schema_json()
    ),
)]
impl Base58Codec {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned value in { "result": ... }.
        match run_skill(&body, "base58-codec", |a: Args| {
            gizza_ai_base58_codec_core::convert(&a.input, &a.mode, &a.variant, &a.format)
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
                    "input": { "type": "string", "description": "The data to encode (text or hex bytes), or the Base58 string to decode." },
                    "mode": { "type": "string", "enum": ["encode", "decode"], "default": "encode", "description": "Direction: 'encode' (default) turns data into Base58, 'decode' reverses it." },
                    "variant": { "type": "string", "enum": ["bitcoin", "ripple", "flickr"], "default": "bitcoin", "description": "Base58 alphabet. 'bitcoin' (default) is the Satoshi/IPFS alphabet (also used by Monero); 'ripple' is the XRP Ledger alphabet; 'flickr' is Flickr's short-URL alphabet (lowercase before uppercase). All exclude the ambiguous 0 O I l and never pad." },
                    "format": { "type": "string", "enum": ["text", "hex"], "default": "text", "description": "How to read the bytes when encoding, or render them when decoding. 'text' (default) is UTF-8; 'hex' is a hex byte string (e.g. '48 65 6c' or '0x48656c') — use 'hex' for binary data that isn't valid UTF-8." }
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
