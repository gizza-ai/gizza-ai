//! gizza-ai/hex-codec — encodes text or bytes to a hexadecimal string and decodes
//! hex back to text. Thin chat-skill wrapper around `gizza-ai-hex-codec-core`. The
//! chat schema is derived from `descriptor()` (single source — shared shape across
//! chat + CLI); the handler delegates to `block_utils::run_skill`. No host calls —
//! runs entirely inside the WASM sandbox.
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
    /// Emit uppercase A–F hex digits when encoding (default false).
    #[serde(default)]
    uppercase: bool,
    #[serde(default)]
    prefix: String,
}

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("The text to encode (read as UTF-8), or the hexadecimal string to decode."),
        )
        .param(
            Param::enumv("mode", ["encode", "decode"])
                .default("encode")
                .describe("Direction: 'encode' (default) turns text into hex, 'decode' turns hex back into text."),
        )
        .param(
            Param::enumv("format", ["text", "bytes"])
                .default("text")
                .describe("On decode, how to render the recovered bytes: 'text' (default) is UTF-8 and errors if the bytes aren't valid UTF-8; 'bytes' shows them as a plain lowercase hex byte string (use for binary data). Ignored on encode."),
        )
        .param(
            Param::enumv("delimiter", ["none", "space", "colon", "dash", "comma", "newline"])
                .default("none")
                .describe("Separator placed between bytes when encoding: 'none' (default), 'space', 'colon' (:), 'dash' (-), 'comma', or 'newline'. Decoding ignores any of these."),
        )
        .param(
            Param::boolean("uppercase")
                .default(false)
                .describe("When true, emit uppercase A–F hex digits while encoding (default false, lowercase). Decoding is case-insensitive."),
        )
        .param(
            Param::enumv("prefix", ["none", "0x", "\\x"])
                .default("none")
                .describe("Marker placed before each byte when encoding: 'none' (default), '0x' (e.g. 0x48), or '\\x' (e.g. \\x48). Decoding strips 0x and \\x automatically."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct HexCodec;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/hex-codec",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Hex encode/decode skill",
    skill(
        description = "Encode text or bytes to a hexadecimal string, or decode a hex string back to text. Use mode='encode' (default, e.g. 'Hi' -> '4869') or mode='decode' (e.g. '4869' -> 'Hi'). delimiter inserts a separator between bytes when encoding ('none' default, 'space', 'colon', 'dash', 'comma', 'newline'); uppercase emits A-F caps; prefix adds '0x' or '\\x' before each byte. On decode, format='text' (default) renders UTF-8 (errors on non-UTF-8 bytes) and format='bytes' shows the raw bytes as a plain hex string. Decoding is case-insensitive and ignores whitespace, the common delimiters (: - ,), and 0x / \\x prefixes, so any encoded form round-trips back.",
        parameters = schema_json()
    ),
)]
impl HexCodec {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned value in { "result": ... }.
        match run_skill(&body, "hex-codec", |a: Args| {
            gizza_ai_hex_codec_core::convert(
                &a.input, &a.mode, &a.format, &a.delimiter, a.uppercase, &a.prefix,
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
                    "input": { "type": "string", "description": "The text to encode (read as UTF-8), or the hexadecimal string to decode." },
                    "mode": { "type": "string", "enum": ["encode", "decode"], "default": "encode", "description": "Direction: 'encode' (default) turns text into hex, 'decode' turns hex back into text." },
                    "format": { "type": "string", "enum": ["text", "bytes"], "default": "text", "description": "On decode, how to render the recovered bytes: 'text' (default) is UTF-8 and errors if the bytes aren't valid UTF-8; 'bytes' shows them as a plain lowercase hex byte string (use for binary data). Ignored on encode." },
                    "delimiter": { "type": "string", "enum": ["none", "space", "colon", "dash", "comma", "newline"], "default": "none", "description": "Separator placed between bytes when encoding: 'none' (default), 'space', 'colon' (:), 'dash' (-), 'comma', or 'newline'. Decoding ignores any of these." },
                    "uppercase": { "type": "boolean", "default": false, "description": "When true, emit uppercase A–F hex digits while encoding (default false, lowercase). Decoding is case-insensitive." },
                    "prefix": { "type": "string", "enum": ["none", "0x", "\\x"], "default": "none", "description": "Marker placed before each byte when encoding: 'none' (default), '0x' (e.g. 0x48), or '\\x' (e.g. \\x48). Decoding strips 0x and \\x automatically." }
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
