//! gizza-ai/hash-all — chat skill block on the shared tool abstraction.
//!
//! Computes EVERY common digest of the same input at once — CRC-32, MD5, SHA-1,
//! SHA-224/256/384/512, SHA3-256/512, RIPEMD-160, BLAKE2b-512, BLAKE2s-256,
//! BLAKE3, and Whirlpool — and returns them as a labeled table. The text can be
//! interpreted as plain UTF-8 (default) or decoded first from hex / base64, and
//! every digest is rendered as lowercase/uppercase hex or base64. Pure Rust
//! (RustCrypto + blake3) → runs on ALL backends including the chat Service
//! Worker. Surfaces: chat + CLI + standalone page.
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
    input_encoding: String,
    #[serde(default)]
    output_format: String,
    #[serde(default)]
    uppercase: bool,
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The text to hash with every algorithm at once."),
        )
        .param(
            Param::enumv("input_encoding", ["text", "hex", "base64"])
                .default("text")
                .describe("How to interpret `text` before hashing. 'text' (default) hashes the UTF-8 bytes as-is; 'hex' decodes it from hexadecimal first; 'base64' decodes it from standard base64 first."),
        )
        .param(
            Param::enumv("output_format", ["hex", "base64"])
                .default("hex")
                .describe("How each digest is rendered. 'hex' (default) is lowercase hex; 'base64' is standard base64."),
        )
        .param(
            Param::boolean("uppercase")
                .default(false)
                .describe("When output_format is hex, emit uppercase hex. No effect on base64. Default false."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/hash-all",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Compute every common digest (MD5/SHA/SHA3/RIPEMD/BLAKE/Whirlpool/CRC-32) of one input at once",
    skill(
        description = "Compute EVERY common digest of the same text at once and return them in a labeled table: CRC-32, MD5, SHA-1, SHA-224, SHA-256, SHA-384, SHA-512, SHA3-256, SHA3-512, RIPEMD-160, BLAKE2b-512, BLAKE2s-256, BLAKE3, and Whirlpool. By default the input is hashed as UTF-8 text and each digest is lowercase hex; set input_encoding='hex' or 'base64' to decode the input to raw bytes before hashing, output_format='base64' for base64 digests, or uppercase=true for uppercase hex. Use this to fingerprint content against any required algorithm, compare checksums, or identify which digest a system expects. To compute a SINGLE chosen algorithm use the hash-text tool; to hash a whole FILE use the file-hash tool.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "hash-all", |a: Args| {
            gizza_ai_hash_all_core::hash_all(
                &a.text,
                &a.input_encoding,
                &a.output_format,
                a.uppercase,
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
                    "text": { "type": "string", "description": "The text to hash with every algorithm at once." },
                    "input_encoding": { "type": "string", "enum": ["text", "hex", "base64"], "default": "text", "description": "How to interpret `text` before hashing. 'text' (default) hashes the UTF-8 bytes as-is; 'hex' decodes it from hexadecimal first; 'base64' decodes it from standard base64 first." },
                    "output_format": { "type": "string", "enum": ["hex", "base64"], "default": "hex", "description": "How each digest is rendered. 'hex' (default) is lowercase hex; 'base64' is standard base64." },
                    "uppercase": { "type": "boolean", "default": false, "description": "When output_format is hex, emit uppercase hex. No effect on base64. Default false." }
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
