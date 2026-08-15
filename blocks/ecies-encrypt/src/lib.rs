//! gizza-ai/ecies-encrypt — ECIES hybrid public-key encryption.
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
    key: String,
    #[serde(default = "default_curve")]
    curve: String,
    #[serde(default = "default_cipher")]
    cipher: String,
    #[serde(default = "default_nonce_length")]
    nonce_length: String,
    #[serde(default)]
    nonce: String,
    #[serde(default)]
    compressed_ephemeral: bool,
    #[serde(default = "default_kdf_input")]
    kdf_input: String,
    #[serde(default)]
    ephemeral_key: String,
    #[serde(default = "default_key_encoding")]
    key_encoding: String,
    #[serde(default = "default_data_encoding")]
    data_encoding: String,
    #[serde(default = "default_output_encoding")]
    output_encoding: String,
}

fn default_operation() -> String {
    "encrypt".to_string()
}
fn default_curve() -> String {
    "secp256k1".to_string()
}
fn default_cipher() -> String {
    "aes-256-gcm".to_string()
}
fn default_nonce_length() -> String {
    "16".to_string()
}
fn default_kdf_input() -> String {
    "ephemeral-and-point".to_string()
}
fn default_key_encoding() -> String {
    "auto".to_string()
}
fn default_data_encoding() -> String {
    "auto".to_string()
}
fn default_output_encoding() -> String {
    "base64".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::enumv("operation", ["encrypt", "decrypt"]).default("encrypt").describe("Operation to perform: encrypt plaintext to a recipient public key, or decrypt an ECIES payload with the matching private key."))
        .param(Param::string("data").required().describe("For encrypt: plaintext (auto/text default) or bytes encoded as hex/base64. For decrypt: the ECIES payload, usually base64 or hex."))
        .param(Param::string("key").required().describe("For encrypt: recipient public key. For decrypt: recipient private key. Accepts SEC1 hex/base64 or PEM when key_encoding=auto/pem."))
        .param(Param::enumv("curve", ["secp256k1", "p256", "p384"]).default("secp256k1").describe("Elliptic curve for the recipient key: secp256k1 (default), p256/prime256v1, or p384."))
        .param(Param::enumv("cipher", ["aes-256-gcm", "xchacha20-poly1305"]).default("aes-256-gcm").describe("Symmetric AEAD used after ECDH+HKDF: AES-256-GCM (default) or XChaCha20-Poly1305."))
        .param(Param::enumv("nonce_length", ["16", "12", "24"]).default("16").describe("Nonce length in bytes. AES-256-GCM accepts 16 (ecies.py/ecies.js default) or 12 (NIST standard); XChaCha20-Poly1305 uses 24."))
        .param(Param::string("nonce").describe("Optional nonce as hex or base64. Leave blank for a fresh random nonce during encryption. Ignored during decryption because the payload carries its nonce."))
        .param(Param::boolean("compressed_ephemeral").default(false).describe("When encrypting, store the ephemeral public key compressed (33/49 bytes) instead of uncompressed SEC1. Decryption auto-detects either shape."))
        .param(Param::enumv("kdf_input", ["ephemeral-and-point", "shared-x"]).default("ephemeral-and-point").describe("HKDF input convention: ephemeral-and-point matches ecies.py/ecies.js; shared-x uses only the ECDH shared X coordinate."))
        .param(Param::string("ephemeral_key").describe("Optional deterministic ephemeral private key for reproducible tests. Leave blank in normal use so a fresh key is generated."))
        .param(Param::enumv("key_encoding", ["auto", "hex", "base64", "pem"]).default("auto").describe("How to decode key and ephemeral_key: auto (detect PEM/hex/base64), hex, base64, or pem."))
        .param(Param::enumv("data_encoding", ["auto", "text", "hex", "base64"]).default("auto").describe("How to decode data. Auto means text while encrypting and base64 while decrypting."))
        .param(Param::enumv("output_encoding", ["base64", "hex"]).default("base64").describe("How to encode binary encryption payloads and non-UTF-8 plaintext on decrypt: base64 (default) or hex."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/ecies-encrypt",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Encrypt or decrypt ECIES messages with secp256k1, P-256, or P-384",
    skill(
        description = "Encrypt or decrypt ECIES hybrid payloads. Encryption accepts a recipient elliptic-curve public key, derives an ephemeral ECDH shared secret, expands it with HKDF-SHA256, and seals data with AES-256-GCM or XChaCha20-Poly1305. Decryption accepts the matching private key and verifies the AEAD tag before returning plaintext. Supports secp256k1, p256 and p384; SEC1 hex/base64 and PEM keys; ecies.py/ecies.js-compatible payload layout; and deterministic nonce/ephemeral-key fields for test vectors.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "ecies-encrypt", |a: Args| {
            gizza_ai_ecies_encrypt_core::run(
                &a.operation,
                &a.data,
                &a.key,
                &a.curve,
                &a.cipher,
                &a.nonce_length,
                &a.nonce,
                a.compressed_ephemeral,
                &a.kdf_input,
                &a.ephemeral_key,
                &a.key_encoding,
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
        assert_eq!(derived["required"], serde_json::json!(["data", "key"]));
        assert_eq!(
            derived["properties"]["operation"]["enum"],
            serde_json::json!(["encrypt", "decrypt"])
        );
        assert_eq!(
            derived["properties"]["curve"]["enum"],
            serde_json::json!(["secp256k1", "p256", "p384"])
        );
        assert_eq!(
            derived["properties"]["cipher"]["enum"],
            serde_json::json!(["aes-256-gcm", "xchacha20-poly1305"])
        );
        assert_eq!(
            derived["properties"]["nonce_length"]["enum"],
            serde_json::json!(["16", "12", "24"])
        );
        assert_eq!(
            derived["properties"]["kdf_input"]["enum"],
            serde_json::json!(["ephemeral-and-point", "shared-x"])
        );
        assert_eq!(
            derived["properties"]["key_encoding"]["enum"],
            serde_json::json!(["auto", "hex", "base64", "pem"])
        );
        assert_eq!(
            derived["properties"]["data_encoding"]["enum"],
            serde_json::json!(["auto", "text", "hex", "base64"])
        );
        assert_eq!(
            derived["properties"]["output_encoding"]["enum"],
            serde_json::json!(["base64", "hex"])
        );
    }
}
