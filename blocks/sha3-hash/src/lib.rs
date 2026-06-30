//! gizza-ai/sha3-hash — chat skill block on the shared tool abstraction.
//!
//! Computes the FIPS-202 **SHA-3** digest (SHA3-256 / SHA3-384 / SHA3-512) of
//! input text. This is the NIST-standardized SHA-3 (`0x06` padding), NOT the
//! original Keccak (`0x01` padding, used by Ethereum) — the two give different
//! digests. The text can be interpreted as plain UTF-8 (default) or decoded
//! first from hex / base64, and the digest is rendered as lowercase/uppercase
//! hex or base64. Pure Rust (RustCrypto `sha3`) → runs on ALL backends including
//! the chat Service Worker. Surfaces: chat + CLI + standalone page.
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
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The text to hash with SHA-3."),
        )
        .param(
            Param::enumv("algorithm", ["sha3-256", "sha3-384", "sha3-512"])
                .default("sha3-256")
                .describe("FIPS-202 SHA-3 variant. 'sha3-256' (default) is a 32-byte digest; 'sha3-384' is 48 bytes; 'sha3-512' is 64 bytes. NOTE: this is the NIST-standardized SHA-3 (0x06 padding), which differs from the original Keccak (0x01 padding) used by Ethereum (use the keccak-hash tool for Keccak-256/512)."),
        )
        .param(
            Param::enumv("input_encoding", ["text", "hex", "base64"])
                .default("text")
                .describe("How to interpret `text` before hashing. 'text' (default) hashes the UTF-8 bytes as-is; 'hex' decodes it from hexadecimal first (a leading 0x is allowed); 'base64' decodes it from standard base64 first."),
        )
        .param(
            Param::enumv("output_format", ["hex", "base64"])
                .default("hex")
                .describe("Digest representation. 'hex' (default) is lowercase hex; 'base64' is standard base64."),
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
    name = "gizza-ai/sha3-hash",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Hash text with FIPS-202 SHA-3 (SHA3-256/384/512)",
    skill(
        description = "Compute the FIPS-202 SHA-3 digest of text — SHA3-256 (default), SHA3-384, or SHA3-512. This is the NIST-standardized SHA-3 (0x06 multi-rate padding), which produces a DIFFERENT digest from the original Keccak (0x01 padding) used throughout Ethereum (for Keccak-256/Keccak-512 use the keccak-hash tool instead). By default the input is hashed as UTF-8 text and the digest is returned as lowercase hex; set input_encoding='hex' (a leading 0x is accepted) or 'base64' to decode the input to raw bytes before hashing, output_format='base64' for a base64 digest, or uppercase=true for uppercase hex.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "sha3-hash", |a: Args| {
            gizza_ai_sha3_hash_core::hash(
                &a.text,
                &a.algorithm,
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
                    "text": { "type": "string", "description": "The text to hash with SHA-3." },
                    "algorithm": { "type": "string", "enum": ["sha3-256", "sha3-384", "sha3-512"], "default": "sha3-256", "description": "FIPS-202 SHA-3 variant. 'sha3-256' (default) is a 32-byte digest; 'sha3-384' is 48 bytes; 'sha3-512' is 64 bytes. NOTE: this is the NIST-standardized SHA-3 (0x06 padding), which differs from the original Keccak (0x01 padding) used by Ethereum (use the keccak-hash tool for Keccak-256/512)." },
                    "input_encoding": { "type": "string", "enum": ["text", "hex", "base64"], "default": "text", "description": "How to interpret `text` before hashing. 'text' (default) hashes the UTF-8 bytes as-is; 'hex' decodes it from hexadecimal first (a leading 0x is allowed); 'base64' decodes it from standard base64 first." },
                    "output_format": { "type": "string", "enum": ["hex", "base64"], "default": "hex", "description": "Digest representation. 'hex' (default) is lowercase hex; 'base64' is standard base64." },
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
