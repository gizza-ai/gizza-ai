//! gizza-ai/generate-pgp-key-pair — generate an OpenPGP key pair (RSA or ECC)
//! with a user ID and optional passphrase, returning armored public + private
//! keys. Pure Rust (rPGP, getrandom CSPRNG) → runs on all backends incl. the
//! chat SW. Surfaces: chat + CLI. No standalone page — a non-deterministic key
//! generator doesn't fit the page's recompute-on-input model (like
//! generate-rsa-key-pair).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_generate_pgp_key_pair_core::{generate, Algo};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    user_id: String,
    #[serde(default = "default_algorithm")]
    algorithm: String,
    #[serde(default)]
    passphrase: String,
}

fn default_algorithm() -> String {
    "curve25519".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("user_id")
                .required()
                .describe("The key's user ID, e.g. \"Alice <alice@example.com>\"."),
        )
        .param(
            Param::enumv("algorithm", ["curve25519", "rsa2048", "rsa3072", "rsa4096"])
                .default("curve25519")
                .describe("Key algorithm: curve25519 (default, fast modern ECC: EdDSA + Curve25519 ECDH) or RSA at 2048/3072/4096 bits."),
        )
        .param(
            Param::string("passphrase")
                .describe("Optional passphrase to encrypt the private key. Omit or leave empty for an unprotected key."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct GeneratePgpKeyPair;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/generate-pgp-key-pair",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Generate an OpenPGP key pair (RSA or ECC)",
    skill(
        description = "Generate a new OpenPGP key pair with the given user ID (e.g. 'Alice <alice@example.com>'). algorithm=curve25519 (default; modern EdDSA signing + Curve25519 ECDH encryption subkey) or rsa2048/rsa3072/rsa4096. Provide an optional passphrase to encrypt the private key. Returns ASCII-armored public and private keys plus the fingerprint. Keys are generated locally with a cryptographic RNG.",
        parameters = schema_json()
    )
)]
impl GeneratePgpKeyPair {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "generate-pgp-key-pair", |a: Args| {
            let algo = Algo::parse(&a.algorithm).map_err(SkillError::InvalidArgs)?;
            let pass = if a.passphrase.is_empty() { None } else { Some(a.passphrase.as_str()) };
            generate(&a.user_id, pass, algo).map_err(SkillError::InvalidArgs)
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
                    "user_id": { "type": "string", "description": "The key's user ID, e.g. \"Alice <alice@example.com>\"." },
                    "algorithm": { "type": "string", "enum": ["curve25519", "rsa2048", "rsa3072", "rsa4096"], "default": "curve25519", "description": "Key algorithm: curve25519 (default, fast modern ECC: EdDSA + Curve25519 ECDH) or RSA at 2048/3072/4096 bits." },
                    "passphrase": { "type": "string", "description": "Optional passphrase to encrypt the private key. Omit or leave empty for an unprotected key." }
                },
                "required": ["user_id"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
