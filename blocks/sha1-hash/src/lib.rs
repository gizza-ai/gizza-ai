//! gizza-ai/sha1-hash — chat skill block on the shared tool abstraction.
//!
//! Computes the SHA-1 digest of input text. The text can be interpreted as
//! plain UTF-8 (default) or decoded first from hex / base64, and the digest is
//! rendered as lowercase/uppercase hex or base64. Pure Rust (RustCrypto `sha1`)
//! → runs on ALL backends including the chat Service Worker. Surfaces: chat +
//! CLI + standalone page.
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
                .describe("The text to hash with SHA-1."),
        )
        .param(
            Param::enumv("input_encoding", ["text", "hex", "base64"])
                .default("text")
                .describe("How to interpret `text` before hashing. 'text' (default) hashes the UTF-8 bytes as-is; 'hex' decodes it from hexadecimal first; 'base64' decodes it from standard base64 first."),
        )
        .param(
            Param::enumv("output_format", ["hex", "base64"])
                .default("hex")
                .describe("Digest representation. 'hex' (default) is 40 lowercase hex chars; 'base64' is standard base64 (28 chars)."),
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
    name = "gizza-ai/sha1-hash",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Compute the SHA-1 digest of text",
    skill(
        description = "Compute the SHA-1 cryptographic hash of text. By default the input is hashed as UTF-8 text and the 160-bit digest is returned as 40 lowercase hex chars. Set input_encoding='hex' or 'base64' to decode the input to raw bytes before hashing (e.g. to hash an existing key or ciphertext). Set output_format='base64' for a base64 digest, or uppercase=true for uppercase hex. NOTE: SHA-1 is cryptographically broken (practical collisions) and must NOT be used for security; use it only for non-security checksums, Git object IDs, and legacy interop — prefer the sha256-hash tool for security. To hash a whole FILE instead of text, use the file-hash tool.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "sha1-hash", |a: Args| {
            gizza_ai_sha1_hash_core::hash(
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
                    "text": { "type": "string", "description": "The text to hash with SHA-1." },
                    "input_encoding": { "type": "string", "enum": ["text", "hex", "base64"], "default": "text", "description": "How to interpret `text` before hashing. 'text' (default) hashes the UTF-8 bytes as-is; 'hex' decodes it from hexadecimal first; 'base64' decodes it from standard base64 first." },
                    "output_format": { "type": "string", "enum": ["hex", "base64"], "default": "hex", "description": "Digest representation. 'hex' (default) is 40 lowercase hex chars; 'base64' is standard base64 (28 chars)." },
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
