//! gizza-ai/nacl-box-encrypt — NaCl crypto_box (Curve25519 + XSalsa20-Poly1305).
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. Pure → runs on all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    #[serde(default = "default_operation")]
    operation: String,
    data: String,
    recipient_key: String,
    sender_key: String,
    #[serde(default)]
    nonce: String,
    #[serde(default = "default_key_encoding")]
    key_encoding: String,
    #[serde(default = "default_nonce_encoding")]
    nonce_encoding: String,
    #[serde(default)]
    data_encoding: String,
    #[serde(default = "default_output_encoding")]
    output_encoding: String,
}

fn default_operation() -> String {
    "encrypt".to_string()
}
fn default_key_encoding() -> String {
    "hex".to_string()
}
fn default_nonce_encoding() -> String {
    "hex".to_string()
}
fn default_output_encoding() -> String {
    "base64".to_string()
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::enumv("operation", ["encrypt", "decrypt"])
                .default("encrypt")
                .describe("Operation to perform: encrypt plaintext for a recipient public key, or decrypt a NaCl box with the recipient secret key."),
        )
        .param(
            Param::string("data")
                .required()
                .describe("For encrypt: plaintext (default data_encoding=text) or bytes encoded as hex/base64. For decrypt: nonce || ciphertext || 16-byte Poly1305 tag (default data_encoding=base64), or ciphertext || tag when nonce is supplied separately."),
        )
        .param(
            Param::string("recipient_key")
                .required()
                .describe("For encrypt: the recipient's 32-byte Curve25519 public key. For decrypt: the recipient's 32-byte secret key. Decoded with key_encoding."),
        )
        .param(
            Param::string("sender_key")
                .required()
                .describe("For encrypt: the sender's 32-byte secret key. For decrypt: the sender's 32-byte public key. Decoded with key_encoding."),
        )
        .param(
            Param::string("nonce")
                .describe("A unique 24-byte nonce. Required for encryption. For decryption it is optional because encrypt output prepends the nonce; provide it only when data contains ciphertext || tag without the nonce prefix."),
        )
        .param(
            Param::enumv("key_encoding", ["hex", "base64"])
                .default("hex")
                .describe("How to decode recipient_key and sender_key: hex (default) or base64. Each key must decode to exactly 32 bytes."),
        )
        .param(
            Param::enumv("nonce_encoding", ["hex", "base64"])
                .default("hex")
                .describe("How to decode nonce: hex (default) or base64. The nonce must decode to exactly 24 bytes."),
        )
        .param(
            Param::enumv("data_encoding", ["text", "hex", "base64"])
                .default("text")
                .describe("How to decode data. Empty/default means text for encryption and base64 for decryption; set hex or base64 explicitly for binary plaintext/ciphertext."),
        )
        .param(
            Param::enumv("output_encoding", ["base64", "hex"])
                .default("base64")
                .describe("How to encode binary output. Encryption returns nonce || ciphertext || tag in base64 (default) or hex. Decryption returns UTF-8 plaintext when valid, otherwise plaintext bytes encoded with this setting."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/nacl-box-encrypt",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Encrypt or decrypt NaCl crypto_box messages with Curve25519 and XSalsa20-Poly1305",
    skill(
        description = "Encrypt or decrypt data with NaCl crypto_box: Curve25519/X25519 key agreement plus XSalsa20-Poly1305 authenticated encryption. operation=encrypt reads plaintext from data, a recipient public key, a sender secret key, and a required 24-byte nonce, then returns nonce || ciphertext || 16-byte tag. operation=decrypt reads the recipient secret key, sender public key, and a combined nonce || ciphertext || tag (or ciphertext || tag plus a separate nonce), verifies the Poly1305 tag, and returns plaintext. Keys support hex/base64 and must be 32 bytes. Runs locally and performs no network access.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "nacl-box-encrypt", |a: Args| {
            gizza_ai_nacl_box_encrypt_core::run(
                &a.operation,
                &a.data,
                &a.recipient_key,
                &a.sender_key,
                &a.nonce,
                &a.key_encoding,
                &a.nonce_encoding,
                &a.data_encoding,
                &a.output_encoding,
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

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(
            derived["required"],
            serde_json::json!(["data", "recipient_key", "sender_key"])
        );
        assert_eq!(
            derived["properties"]["operation"]["enum"],
            serde_json::json!(["encrypt", "decrypt"])
        );
        assert_eq!(
            derived["properties"]["key_encoding"]["enum"],
            serde_json::json!(["hex", "base64"])
        );
        assert_eq!(
            derived["properties"]["nonce_encoding"]["enum"],
            serde_json::json!(["hex", "base64"])
        );
        assert_eq!(
            derived["properties"]["data_encoding"]["enum"],
            serde_json::json!(["text", "hex", "base64"])
        );
        assert_eq!(
            derived["properties"]["output_encoding"]["enum"],
            serde_json::json!(["base64", "hex"])
        );
    }
}
