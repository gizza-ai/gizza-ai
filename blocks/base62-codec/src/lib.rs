//! gizza-ai/base62-codec — encodes text, hex bytes, or a decimal number to
//! Base62 (the `0-9A-Za-z` alphabet, plus the `0-9a-zA-Z` inverted variant) and
//! decodes Base62 back to the original data, preserving leading-zero bytes.
//!
//! Thin chat-skill wrapper around `gizza-ai-base62-codec-core`. The chat schema
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
                .describe("The data to encode (text, hex bytes, or a decimal number), or the Base62 string to decode."),
        )
        .param(
            Param::enumv("mode", ["encode", "decode"])
                .default("encode")
                .describe("Direction: 'encode' (default) turns data into Base62, 'decode' reverses it."),
        )
        .param(
            Param::enumv("variant", ["standard", "inverted"])
                .default("standard")
                .describe("Base62 alphabet order. 'standard' (default) is 0-9A-Za-z (digits, then uppercase, then lowercase — the GMP order used by most short-ID libraries); 'inverted' is 0-9a-zA-Z (lowercase before uppercase). Base62 has no padding and no ambiguous-character exclusions."),
        )
        .param(
            Param::enumv("format", ["text", "hex", "number"])
                .default("text")
                .describe("How to read the input when encoding, or render the result when decoding. 'text' (default) is UTF-8; 'hex' is a hex byte string (e.g. '48 65 6c' or '0x48656c') for binary data; 'number' is a non-negative decimal integer of any size (encoded with no leading-zero padding)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Base62Codec;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/base62-codec",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Base62 encode/decode skill",
    skill(
        description = "Encode text, hex bytes, or a decimal number to Base62, or decode a Base62 string back to the original data. Base62 uses the 62 alphanumerics (0-9, A-Z, a-z) with no padding and no special characters, so its strings are compact and safe in URLs — it's commonly used for short IDs and URL slugs. Use mode='encode' (default) or mode='decode'. variant selects the alphabet order: 'standard' (default) is 0-9A-Za-z (e.g. 'Hello World!' -> 'T8dgcjRGkZ3aysdN'; the number 12345 -> '3D7'); 'inverted' is 0-9a-zA-Z. format='text' (default) treats the data as UTF-8; format='hex' reads/writes a hex byte string for binary data; format='number' reads/writes a non-negative decimal integer of arbitrary size. Byte encoding preserves leading-zero bytes as leading '0' symbols.",
        parameters = schema_json()
    ),
)]
impl Base62Codec {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned value in { "result": ... }.
        match run_skill(&body, "base62-codec", |a: Args| {
            gizza_ai_base62_codec_core::convert(&a.input, &a.mode, &a.variant, &a.format)
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
                    "input": { "type": "string", "description": "The data to encode (text, hex bytes, or a decimal number), or the Base62 string to decode." },
                    "mode": { "type": "string", "enum": ["encode", "decode"], "default": "encode", "description": "Direction: 'encode' (default) turns data into Base62, 'decode' reverses it." },
                    "variant": { "type": "string", "enum": ["standard", "inverted"], "default": "standard", "description": "Base62 alphabet order. 'standard' (default) is 0-9A-Za-z (digits, then uppercase, then lowercase — the GMP order used by most short-ID libraries); 'inverted' is 0-9a-zA-Z (lowercase before uppercase). Base62 has no padding and no ambiguous-character exclusions." },
                    "format": { "type": "string", "enum": ["text", "hex", "number"], "default": "text", "description": "How to read the input when encoding, or render the result when decoding. 'text' (default) is UTF-8; 'hex' is a hex byte string (e.g. '48 65 6c' or '0x48656c') for binary data; 'number' is a non-negative decimal integer of any size (encoded with no leading-zero padding)." }
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
