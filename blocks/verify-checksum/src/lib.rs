//! gizza-ai/verify-checksum — chat skill block on the shared tool abstraction.
//!
//! Verifies that a piece of input data matches an expected checksum. The input
//! text is hashed (as UTF-8 by default, or decoded from hex/base64 first) and
//! the digest is compared against the supplied expected checksum (hex or
//! base64). The algorithm can be given explicitly or auto-detected from the
//! expected digest's byte length. Pure Rust (RustCrypto + blake3) → runs on ALL
//! backends including the chat Service Worker. Surfaces: chat + CLI + page.
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
    expected: String,
    #[serde(default)]
    algorithm: String,
    #[serde(default)]
    input_encoding: String,
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The input data to hash and check."),
        )
        .param(
            Param::string("expected")
                .required()
                .describe("The expected checksum to compare against, as hex (optionally 0x-prefixed, any case) or standard base64."),
        )
        .param(
            Param::enumv(
                "algorithm",
                [
                    "auto", "md5", "sha1", "sha224", "sha256", "sha384", "sha512", "sha3-256",
                    "sha3-512", "blake2b-512", "blake2s-256", "blake3",
                ],
            )
            .default("auto")
            .describe("Hash algorithm. 'auto' (default) infers it from the expected digest's byte length, trying every algorithm of that width. Otherwise pick one explicitly: 'md5'/'sha1' are legacy checksums; 'sha256' is the modern standard; 'sha3-256'/'sha3-512' are SHA-3; 'blake2b-512'/'blake2s-256'/'blake3' are fast modern hashes."),
        )
        .param(
            Param::enumv("input_encoding", ["text", "hex", "base64"])
                .default("text")
                .describe("How to interpret `text` before hashing. 'text' (default) hashes the UTF-8 bytes as-is; 'hex' decodes it from hexadecimal first; 'base64' decodes it from standard base64 first."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/verify-checksum",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Verify that data matches an expected checksum (auto-detects the algorithm)",
    skill(
        description = "Verify that a piece of data matches an expected checksum. Provide the input `text` and the `expected` checksum (hex, optionally 0x-prefixed and any case, or standard base64); the tool hashes the input and reports MATCH or MISMATCH along with the algorithm used and the computed digest. By default `algorithm`='auto' infers the hash from the expected digest's byte length (e.g. 32 hex chars → MD5, 64 → SHA-256/SHA3-256/BLAKE2s/BLAKE3); set it explicitly (md5, sha1, sha224, sha256, sha384, sha512, sha3-256, sha3-512, blake2b-512, blake2s-256, blake3) to check one algorithm. Set input_encoding='hex' or 'base64' to decode the input to raw bytes before hashing. To just compute a hash use the hash-text tool; to identify an unknown hash's format use the hash-identifier tool.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "verify-checksum", |a: Args| {
            gizza_ai_verify_checksum_core::verify_report(
                &a.text,
                &a.expected,
                &a.algorithm,
                &a.input_encoding,
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
                    "text": { "type": "string", "description": "The input data to hash and check." },
                    "expected": { "type": "string", "description": "The expected checksum to compare against, as hex (optionally 0x-prefixed, any case) or standard base64." },
                    "algorithm": { "type": "string", "enum": ["auto", "md5", "sha1", "sha224", "sha256", "sha384", "sha512", "sha3-256", "sha3-512", "blake2b-512", "blake2s-256", "blake3"], "default": "auto", "description": "Hash algorithm. 'auto' (default) infers it from the expected digest's byte length, trying every algorithm of that width. Otherwise pick one explicitly: 'md5'/'sha1' are legacy checksums; 'sha256' is the modern standard; 'sha3-256'/'sha3-512' are SHA-3; 'blake2b-512'/'blake2s-256'/'blake3' are fast modern hashes." },
                    "input_encoding": { "type": "string", "enum": ["text", "hex", "base64"], "default": "text", "description": "How to interpret `text` before hashing. 'text' (default) hashes the UTF-8 bytes as-is; 'hex' decodes it from hexadecimal first; 'base64' decodes it from standard base64 first." }
                },
                "required": ["text", "expected"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
