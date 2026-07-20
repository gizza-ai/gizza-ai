//! gizza-ai/ecdsa-secp256k1 — generate secp256k1 keypairs and sign/verify
//! messages with ECDSA (the Bitcoin/Ethereum curve). Chat schema single-sourced
//! from descriptor(); handler delegates to run_skill. Pure-Rust k256; signing is
//! deterministic (RFC 6979) and keygen uses WASI random_get → runs on all
//! backends. Surfaces: chat + CLI + page.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_ecdsa_secp256k1_core::{process, HashAlg, MsgEncoding, Operation};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
#[serde(default)]
struct Args {
    operation: String,
    message: String,
    message_encoding: String,
    hash: String,
    key: String,
    signature: String,
}

impl Default for Args {
    fn default() -> Self {
        Args {
            operation: "generate".into(),
            message: String::new(),
            message_encoding: "utf8".into(),
            hash: "sha256".into(),
            key: String::new(),
            signature: String::new(),
        }
    }
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::enumv("operation", ["generate", "sign", "verify"]).default("generate").describe(
                "generate: create a fresh secp256k1 keypair (needs no other fields). sign: create an ECDSA signature for the message with a private key. verify: check a signature against the message with the signer's public key.",
            ),
        )
        .param(Param::string("message").describe(
            "The message to sign or verify (ignored for generate). Interpreted using message_encoding, then hashed with hash before ECDSA.",
        ))
        .param(
            Param::enumv("message_encoding", ["utf8", "hex", "base64"]).default("utf8").describe(
                "How to read the message into bytes: utf8 (plain text), hex (0x optional), or base64 (for binary payloads).",
            ),
        )
        .param(
            Param::enumv("hash", ["sha256", "keccak256", "sha384", "sha512", "none"])
                .default("sha256")
                .describe(
                    "Digest applied to the message before signing/verifying: sha256 (default, Bitcoin-style), keccak256 (Ethereum), sha384, sha512, or none (the message IS the 32-byte digest, e.g. a transaction hash as hex).",
                ),
        )
        .param(Param::string("key").describe(
            "For sign: your secp256k1 private key — raw 32 bytes as hex (0x optional) or base64, or PEM (SEC1 'EC PRIVATE KEY' / PKCS#8 'PRIVATE KEY'). For verify: the signer's public key — SEC1 point, 33-byte compressed (02/03…) or 65-byte uncompressed (04…), as hex or base64, or SPKI PEM. Ignored for generate.",
        ))
        .param(Param::string("signature").describe(
            "Required for verify: the signature to check — compact 64-byte r||s or ASN.1 DER, as hex or base64. Ignored for generate and sign.",
        ))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct EcdsaSecp256k1;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/ecdsa-secp256k1",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Generate secp256k1 keypairs and sign or verify messages with ECDSA",
    skill(
        description = "Generate a secp256k1 keypair, or sign/verify a message with ECDSA — the signature scheme behind Bitcoin and Ethereum keys. Signing is deterministic (RFC 6979) and low-S normalized, and returns the signature as compact r||s (hex + base64) and ASN.1 DER, plus the r/s components, the recovery id (and Ethereum-style v = 27 + id), and the derived public key. Verifying accepts compact or DER signatures and returns whether the signature is valid. Keys are accepted as raw hex (0x optional), base64, or PEM (SEC1/PKCS#8 private, SPKI public); messages can be utf8 text, hex, or base64, hashed with SHA-256 (default), Keccak-256 (Ethereum), SHA-384, SHA-512, or signed as a pre-hashed 32-byte digest (hash=none). Runs locally — keys and messages never leave the device.",
        parameters = schema_json()
    ),
)]
impl EcdsaSecp256k1 {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "ecdsa-secp256k1", |a: Args| {
            let op = Operation::parse(&a.operation).map_err(SkillError::InvalidArgs)?;
            let enc = MsgEncoding::parse(&a.message_encoding).map_err(SkillError::InvalidArgs)?;
            let hash = HashAlg::parse(&a.hash).map_err(SkillError::InvalidArgs)?;
            process(op, &a.message, enc, hash, &a.key, &a.signature)
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
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "operation": { "type": "string", "enum": ["generate", "sign", "verify"], "default": "generate", "description": "generate: create a fresh secp256k1 keypair (needs no other fields). sign: create an ECDSA signature for the message with a private key. verify: check a signature against the message with the signer's public key." },
                    "message": { "type": "string", "description": "The message to sign or verify (ignored for generate). Interpreted using message_encoding, then hashed with hash before ECDSA." },
                    "message_encoding": { "type": "string", "enum": ["utf8", "hex", "base64"], "default": "utf8", "description": "How to read the message into bytes: utf8 (plain text), hex (0x optional), or base64 (for binary payloads)." },
                    "hash": { "type": "string", "enum": ["sha256", "keccak256", "sha384", "sha512", "none"], "default": "sha256", "description": "Digest applied to the message before signing/verifying: sha256 (default, Bitcoin-style), keccak256 (Ethereum), sha384, sha512, or none (the message IS the 32-byte digest, e.g. a transaction hash as hex)." },
                    "key": { "type": "string", "description": "For sign: your secp256k1 private key — raw 32 bytes as hex (0x optional) or base64, or PEM (SEC1 'EC PRIVATE KEY' / PKCS#8 'PRIVATE KEY'). For verify: the signer's public key — SEC1 point, 33-byte compressed (02/03…) or 65-byte uncompressed (04…), as hex or base64, or SPKI PEM. Ignored for generate." },
                    "signature": { "type": "string", "description": "Required for verify: the signature to check — compact 64-byte r||s or ASN.1 DER, as hex or base64. Ignored for generate and sign." }
                },
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
