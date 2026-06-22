//! gizza-ai/hash-text — chat skill block on the shared tool abstraction.
//!
//! Computes a cryptographic hash of input text with a selectable algorithm
//! (MD5, SHA-1, SHA-2 family, SHA-3, BLAKE2, BLAKE3). The text can be
//! interpreted as plain UTF-8 (default) or decoded first from hex / base64, and
//! the digest is rendered as lowercase/uppercase hex or base64. Pure Rust
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
                .describe("The text to hash."),
        )
        .param(
            Param::enumv(
                "algorithm",
                [
                    "md5", "sha1", "sha224", "sha256", "sha384", "sha512", "sha3-256",
                    "sha3-512", "blake2b-512", "blake2s-256", "blake3",
                ],
            )
            .default("sha256")
            .describe("Hash algorithm. 'sha256' (default) is the standard modern choice; 'md5'/'sha1' are legacy/non-cryptographic (checksums only); 'sha3-256'/'sha3-512' are the Keccak SHA-3 family; 'blake2b-512'/'blake2s-256'/'blake3' are fast modern hashes."),
        )
        .param(
            Param::enumv("input_encoding", ["text", "hex", "base64"])
                .default("text")
                .describe("How to interpret `text` before hashing. 'text' (default) hashes the UTF-8 bytes as-is; 'hex' decodes it from hexadecimal first; 'base64' decodes it from standard base64 first."),
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
    name = "gizza-ai/hash-text",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Hash text with a selectable algorithm (MD5/SHA-1/SHA-2/SHA-3/BLAKE)",
    skill(
        description = "Compute a cryptographic hash of text with a selectable algorithm. Supports MD5, SHA-1, SHA-224, SHA-256 (default), SHA-384, SHA-512, SHA3-256, SHA3-512, BLAKE2b-512, BLAKE2s-256, and BLAKE3. By default the input is hashed as UTF-8 text and the digest is returned as lowercase hex; set input_encoding='hex' or 'base64' to decode the input to raw bytes before hashing, output_format='base64' for a base64 digest, or uppercase=true for uppercase hex. Use this to verify integrity, fingerprint content, or derive identifiers. To hash a whole FILE (and get several digests at once) use the file-hash tool.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "hash-text", |a: Args| {
            gizza_ai_hash_text_core::hash(
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
                    "text": { "type": "string", "description": "The text to hash." },
                    "algorithm": { "type": "string", "enum": ["md5", "sha1", "sha224", "sha256", "sha384", "sha512", "sha3-256", "sha3-512", "blake2b-512", "blake2s-256", "blake3"], "default": "sha256", "description": "Hash algorithm. 'sha256' (default) is the standard modern choice; 'md5'/'sha1' are legacy/non-cryptographic (checksums only); 'sha3-256'/'sha3-512' are the Keccak SHA-3 family; 'blake2b-512'/'blake2s-256'/'blake3' are fast modern hashes." },
                    "input_encoding": { "type": "string", "enum": ["text", "hex", "base64"], "default": "text", "description": "How to interpret `text` before hashing. 'text' (default) hashes the UTF-8 bytes as-is; 'hex' decodes it from hexadecimal first; 'base64' decodes it from standard base64 first." },
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
