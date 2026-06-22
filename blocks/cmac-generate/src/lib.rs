//! gizza-ai/cmac-generate — chat skill block on the shared tool abstraction.
//!
//! Computes an AES-CMAC (Cipher-based Message Authentication Code, NIST SP
//! 800-38B / RFC 4493) over a message with a 128/192/256-bit key. The AES
//! variant is chosen by the key length (16/24/32 bytes). The message and key can
//! each be read as UTF-8 text (default) or decoded first from hex / base64, and
//! the 16-byte tag is rendered as lowercase/uppercase hex or base64. Pure Rust
//! (RustCrypto) → runs on ALL backends including the chat Service Worker.
//! Surfaces: chat + CLI + standalone page.
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
                .describe("The message to authenticate. An empty message is allowed."),
        )
        .param(
            Param::string("key")
                .required()
                .describe("The secret AES key. Its length selects the AES variant: 16 bytes → AES-128, 24 → AES-192, 32 → AES-256. Interpreted per key_encoding (UTF-8 text by default); use hex/base64 for a binary key."),
        )
        .param(
            Param::enumv("message_encoding", ["text", "hex", "base64"])
                .default("text")
                .describe("How to interpret `message` before MAC-ing. 'text' (default) uses the UTF-8 bytes as-is; 'hex' decodes from hexadecimal first; 'base64' decodes from standard base64 first."),
        )
        .param(
            Param::enumv("key_encoding", ["text", "hex", "base64"])
                .default("text")
                .describe("How to interpret `key`. 'text' (default) uses the UTF-8 bytes as-is; 'hex' decodes from hexadecimal first; 'base64' decodes from standard base64 first. Use hex/base64 for binary keys (a 16-byte AES-128 key is 32 hex chars)."),
        )
        .param(
            Param::enumv("output_format", ["hex", "base64"])
                .default("hex")
                .describe("Tag representation. 'hex' (default) is lowercase hex; 'base64' is standard base64. The tag is always 16 bytes (128 bits)."),
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
    name = "gizza-ai/cmac-generate",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Compute an AES-CMAC (block-cipher MAC) over a message with a 128/192/256-bit key",
    skill(
        description = "Compute an AES-CMAC (Cipher-based Message Authentication Code, NIST SP 800-38B / RFC 4493) of a message with a secret AES key. CMAC is a block-cipher MAC built on AES; the AES variant is selected by the key length — 16 bytes for AES-128, 24 for AES-192, 32 for AES-256 — and the tag is always 16 bytes. By default the message and key are read as UTF-8 text and the tag is returned as lowercase hex; set message_encoding/key_encoding to 'hex' or 'base64' to decode binary inputs first (a 16-byte key is 32 hex chars), output_format='base64' for a base64 tag, or uppercase=true for uppercase hex. Use this to authenticate messages with a symmetric key when a block cipher (rather than a hash) is the available primitive — e.g. AES-CMAC in IoT/smart-card and IEEE 802.1X / RFC 4493 contexts. For a HASH-based keyed MAC use the hmac-generate tool instead.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "cmac-generate", |a: Args| {
            gizza_ai_cmac_generate_core::cmac(
                &a.message,
                &a.key,
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
                    "message": { "type": "string", "description": "The message to authenticate. An empty message is allowed." },
                    "key": { "type": "string", "description": "The secret AES key. Its length selects the AES variant: 16 bytes → AES-128, 24 → AES-192, 32 → AES-256. Interpreted per key_encoding (UTF-8 text by default); use hex/base64 for a binary key." },
                    "message_encoding": { "type": "string", "enum": ["text", "hex", "base64"], "default": "text", "description": "How to interpret `message` before MAC-ing. 'text' (default) uses the UTF-8 bytes as-is; 'hex' decodes from hexadecimal first; 'base64' decodes from standard base64 first." },
                    "key_encoding": { "type": "string", "enum": ["text", "hex", "base64"], "default": "text", "description": "How to interpret `key`. 'text' (default) uses the UTF-8 bytes as-is; 'hex' decodes from hexadecimal first; 'base64' decodes from standard base64 first. Use hex/base64 for binary keys (a 16-byte AES-128 key is 32 hex chars)." },
                    "output_format": { "type": "string", "enum": ["hex", "base64"], "default": "hex", "description": "Tag representation. 'hex' (default) is lowercase hex; 'base64' is standard base64. The tag is always 16 bytes (128 bits)." },
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
