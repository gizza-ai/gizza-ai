//! gizza-ai/lz-string-compress — compress/decompress text with LZ-String.
//!
//! Thin chat-skill wrapper around `gizza-ai-lz-string-compress-core`. The chat
//! schema is derived from `descriptor()` (single source — shared shape across
//! chat + CLI); the handler delegates to `block_utils::run_skill`. No host
//! calls — runs entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default)]
    mode: String,
    #[serde(default)]
    format: String,
}

/// Single-source param descriptor → chat schema (and CLI). See
/// docs/superpowers/specs/2026-06-19-gizza-shared-tool-abstraction-design.md.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The text to compress, or an LZ-String payload to decompress."),
        )
        .param(
            Param::enumv("mode", ["compress", "decompress"])
                .default("compress")
                .describe("Direction: 'compress' (default) shrinks text with LZ-String; 'decompress' restores the original text from a payload."),
        )
        .param(
            Param::enumv("format", ["base64", "uri", "utf16"])
                .default("base64")
                .describe("Encoding of the compressed payload. 'base64' (default) is portable ASCII (matches LZString.compressToBase64); 'uri' is URL-query-safe (no +/= — drop straight into a ?param= value); 'utf16' packs 15 bits per character, most compact for localStorage. When decompressing, this must match how the payload was produced."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct LzStringCompress;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/lz-string-compress",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "LZ-String compress / decompress",
    skill(
        description = "Compress or decompress text with LZ-String (pieroxy's lz-string), the LZW variant made for fitting data into URLs and browser storage. Use mode='compress' (default) to shrink text or mode='decompress' to restore it. Choose format='base64' (default, portable ASCII, byte-identical to LZString.compressToBase64), format='uri' (URL-query-safe, no + / = — paste straight into a ?param= value), or format='utf16' (packs 15 bits per character, most compact for localStorage). When decompressing, format must match how the payload was produced. Output is byte-compatible with the original lz-string JavaScript library.",
        parameters = schema_json()
    ),
)]
impl LzStringCompress {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned value in { "result": … }.
        match run_skill(&body, "lz-string-compress", |a: Args| {
            gizza_ai_lz_string_compress_core::convert(&a.text, &a.mode, &a.format)
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
                    "text": { "type": "string", "description": "The text to compress, or an LZ-String payload to decompress." },
                    "mode": { "type": "string", "enum": ["compress", "decompress"], "default": "compress", "description": "Direction: 'compress' (default) shrinks text with LZ-String; 'decompress' restores the original text from a payload." },
                    "format": { "type": "string", "enum": ["base64", "uri", "utf16"], "default": "base64", "description": "Encoding of the compressed payload. 'base64' (default) is portable ASCII (matches LZString.compressToBase64); 'uri' is URL-query-safe (no +/= — drop straight into a ?param= value); 'utf16' packs 15 bits per character, most compact for localStorage. When decompressing, this must match how the payload was produced." }
                },
                "required": ["text"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
