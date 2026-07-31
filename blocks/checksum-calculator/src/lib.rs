//! gizza-ai/checksum-calculator — chat skill block on the shared tool abstraction.
//!
//! Computes a CRC-family checksum — CRC-32 (zip/gzip/PNG/Ethernet), CRC-32C
//! (iSCSI/ext4/SSE4.2), CRC-16 (CRC-16/ARC), or CRC-8 (CRC-8/SMBUS) — of an
//! input, and optionally verifies it against an expected value. The input can
//! be plain UTF-8 text (default) or decoded first from hex / base64 so raw file
//! bytes can be checksummed, and the value is rendered as hex (default) or
//! decimal. Pure Rust computed from each CRC's canonical parameters → runs on
//! ALL backends including the chat Service Worker. Surfaces: chat + CLI + page.
//!
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default)]
    algorithm: String,
    #[serde(default)]
    input_encoding: String,
    #[serde(default)]
    output_format: String,
    #[serde(default)]
    uppercase: bool,
    #[serde(default)]
    expected: String,
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The text (or hex/base64-encoded bytes) to compute the checksum of."),
        )
        .param(
            Param::enumv("algorithm", ["crc32", "crc32c", "crc16", "crc8"])
                .default("crc32")
                .describe("Which CRC to compute. 'crc32' (default) is CRC-32/ISO-HDLC used by zip/gzip/PNG/Ethernet; 'crc32c' is CRC-32C/Castagnoli used by iSCSI/ext4/SSE4.2; 'crc16' is CRC-16/ARC (the classic 'CRC-16'); 'crc8' is CRC-8/SMBUS (the plain 'CRC-8')."),
        )
        .param(
            Param::enumv("input_encoding", ["text", "hex", "base64"])
                .default("text")
                .describe("How to interpret `text` before checksumming. 'text' (default) uses the UTF-8 bytes as-is; 'hex' decodes it from hexadecimal first; 'base64' decodes it from standard base64 first (so raw file bytes can be checksummed)."),
        )
        .param(
            Param::enumv("output_format", ["hex", "decimal"])
                .default("hex")
                .describe("How the checksum value is rendered. 'hex' (default) is zero-padded hexadecimal; 'decimal' is an unsigned integer."),
        )
        .param(
            Param::boolean("uppercase")
                .default(false)
                .describe("When output_format is hex, emit uppercase hex. No effect on decimal. Default false."),
        )
        .param(
            Param::string("expected")
                .default("")
                .describe("Optional expected checksum to verify against. When non-empty, the result reports MATCH or MISMATCH. Accepts hex (with or without a leading 0x, any case, leading zeros ignored) or a plain decimal integer."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/checksum-calculator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Compute a CRC checksum (CRC-32, CRC-32C, CRC-16 or CRC-8) of text or encoded bytes, and optionally verify it",
    skill(
        description = "Compute a CRC-family checksum of an input and optionally verify it against an expected value. Choose algorithm='crc32' (default, CRC-32/ISO-HDLC used by zip/gzip/PNG/Ethernet), 'crc32c' (CRC-32C/Castagnoli used by iSCSI/ext4/SSE4.2), 'crc16' (CRC-16/ARC), or 'crc8' (CRC-8/SMBUS). By default `text` is checksummed as UTF-8 and the value is lowercase hex; set input_encoding='hex' or 'base64' to decode the input to raw bytes first, output_format='decimal' for an integer, or uppercase=true for uppercase hex. Provide `expected` to check a known checksum and get a MATCH/MISMATCH verdict. Use this for error-detection checksums like file/packet integrity; for cryptographic digests use hash-text or hash-all, and to verify a cryptographic checksum use verify-checksum.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "checksum-calculator", |a: Args| {
            gizza_ai_checksum_calculator_core::checksum(
                &a.text,
                &a.algorithm,
                &a.input_encoding,
                &a.output_format,
                a.uppercase,
                &a.expected,
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
    /// schema, so any future change to the LLM-facing API is intentional.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "text": { "type": "string", "description": "The text (or hex/base64-encoded bytes) to compute the checksum of." },
                    "algorithm": { "type": "string", "enum": ["crc32", "crc32c", "crc16", "crc8"], "default": "crc32", "description": "Which CRC to compute. 'crc32' (default) is CRC-32/ISO-HDLC used by zip/gzip/PNG/Ethernet; 'crc32c' is CRC-32C/Castagnoli used by iSCSI/ext4/SSE4.2; 'crc16' is CRC-16/ARC (the classic 'CRC-16'); 'crc8' is CRC-8/SMBUS (the plain 'CRC-8')." },
                    "input_encoding": { "type": "string", "enum": ["text", "hex", "base64"], "default": "text", "description": "How to interpret `text` before checksumming. 'text' (default) uses the UTF-8 bytes as-is; 'hex' decodes it from hexadecimal first; 'base64' decodes it from standard base64 first (so raw file bytes can be checksummed)." },
                    "output_format": { "type": "string", "enum": ["hex", "decimal"], "default": "hex", "description": "How the checksum value is rendered. 'hex' (default) is zero-padded hexadecimal; 'decimal' is an unsigned integer." },
                    "uppercase": { "type": "boolean", "default": false, "description": "When output_format is hex, emit uppercase hex. No effect on decimal. Default false." },
                    "expected": { "type": "string", "default": "", "description": "Optional expected checksum to verify against. When non-empty, the result reports MATCH or MISMATCH. Accepts hex (with or without a leading 0x, any case, leading zeros ignored) or a plain decimal integer." }
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
