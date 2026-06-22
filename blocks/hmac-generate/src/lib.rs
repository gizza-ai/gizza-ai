//! gizza-ai/hmac-generate — chat skill block on the shared tool abstraction.
//!
//! Computes a keyed-hash message authentication code (HMAC) of a message with a
//! selectable underlying hash (HMAC-SHA256 default, plus SHA-1, SHA-224/384/512,
//! SHA3-256/512 and MD5). The message and key can each be interpreted as plain
//! UTF-8 (default) or decoded first from hex / base64, and the tag is rendered
//! as lowercase/uppercase hex or base64. Pure Rust (RustCrypto) → runs on ALL
//! backends including the chat Service Worker. Surfaces: chat + CLI + standalone page.
//!
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    message: String,
    key: String,
    #[serde(default)]
    algorithm: String,
    #[serde(default)]
    message_encoding: String,
    #[serde(default)]
    key_encoding: String,
    #[serde(default)]
    output_format: String,
    #[serde(default)]
    uppercase: bool,
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("message")
                .required()
                .describe("The message to authenticate."),
        )
        .param(
            Param::string("key")
                .required()
                .describe("The secret key. Interpreted per key_encoding (UTF-8 text by default). An empty string is allowed."),
        )
        .param(
            Param::enumv(
                "algorithm",
                ["md5", "sha1", "sha224", "sha256", "sha384", "sha512", "sha3-256", "sha3-512"],
            )
            .default("sha256")
            .describe("Underlying hash for the HMAC. 'sha256' (default) is the standard modern choice; 'sha1' and 'md5' are legacy (HMAC-SHA1/HMAC-MD5 are still seen in older APIs); 'sha224'/'sha384'/'sha512' are the wider SHA-2 family; 'sha3-256'/'sha3-512' are the Keccak SHA-3 family."),
        )
        .param(
            Param::enumv("message_encoding", ["text", "hex", "base64"])
                .default("text")
                .describe("How to interpret `message` before MAC-ing. 'text' (default) uses the UTF-8 bytes as-is; 'hex' decodes from hexadecimal first; 'base64' decodes from standard base64 first."),
        )
        .param(
            Param::enumv("key_encoding", ["text", "hex", "base64"])
                .default("text")
                .describe("How to interpret `key`. 'text' (default) uses the UTF-8 bytes as-is; 'hex' decodes from hexadecimal first; 'base64' decodes from standard base64 first. Use hex/base64 for binary keys."),
        )
        .param(
            Param::enumv("output_format", ["hex", "base64"])
                .default("hex")
                .describe("Tag representation. 'hex' (default) is lowercase hex; 'base64' is standard base64."),
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
    name = "gizza-ai/hmac-generate",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Compute an HMAC (keyed hash) of a message with a selectable hash (HMAC-SHA256/SHA1/SHA512/…)",
    skill(
        description = "Compute a keyed-hash message authentication code (HMAC) of a message with a secret key and a selectable underlying hash. Supports HMAC-MD5, HMAC-SHA1, HMAC-SHA224, HMAC-SHA256 (default), HMAC-SHA384, HMAC-SHA512, HMAC-SHA3-256 and HMAC-SHA3-512. By default the message and key are read as UTF-8 text and the tag is returned as lowercase hex; set message_encoding/key_encoding to 'hex' or 'base64' to decode binary inputs first, output_format='base64' for a base64 tag, or uppercase=true for uppercase hex. Use this to sign API requests, verify webhook signatures (e.g. Stripe/GitHub), or derive a key-bound integrity tag. For an UNKEYED hash (no secret key) use the hash-text tool instead.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "hmac-generate", |a: Args| {
            gizza_ai_hmac_generate_core::hmac(
                &a.message,
                &a.key,
                &a.algorithm,
                &a.message_encoding,
                &a.key_encoding,
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
                    "message": { "type": "string", "description": "The message to authenticate." },
                    "key": { "type": "string", "description": "The secret key. Interpreted per key_encoding (UTF-8 text by default). An empty string is allowed." },
                    "algorithm": { "type": "string", "enum": ["md5", "sha1", "sha224", "sha256", "sha384", "sha512", "sha3-256", "sha3-512"], "default": "sha256", "description": "Underlying hash for the HMAC. 'sha256' (default) is the standard modern choice; 'sha1' and 'md5' are legacy (HMAC-SHA1/HMAC-MD5 are still seen in older APIs); 'sha224'/'sha384'/'sha512' are the wider SHA-2 family; 'sha3-256'/'sha3-512' are the Keccak SHA-3 family." },
                    "message_encoding": { "type": "string", "enum": ["text", "hex", "base64"], "default": "text", "description": "How to interpret `message` before MAC-ing. 'text' (default) uses the UTF-8 bytes as-is; 'hex' decodes from hexadecimal first; 'base64' decodes from standard base64 first." },
                    "key_encoding": { "type": "string", "enum": ["text", "hex", "base64"], "default": "text", "description": "How to interpret `key`. 'text' (default) uses the UTF-8 bytes as-is; 'hex' decodes from hexadecimal first; 'base64' decodes from standard base64 first. Use hex/base64 for binary keys." },
                    "output_format": { "type": "string", "enum": ["hex", "base64"], "default": "hex", "description": "Tag representation. 'hex' (default) is lowercase hex; 'base64' is standard base64." },
                    "uppercase": { "type": "boolean", "default": false, "description": "When output_format is hex, emit uppercase hex. No effect on base64. Default false." }
                },
                "required": ["message", "key"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
