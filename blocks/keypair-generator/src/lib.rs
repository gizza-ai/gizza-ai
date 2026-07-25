//! gizza-ai/keypair-generator — chat skill block on the shared tool abstraction.
//! Generates an X25519 or Ed25519 key pair and returns it in hex, base64, and
//! PEM. The chat schema is single-sourced from descriptor() (which also drives
//! the CLI); handle() delegates to block_utils::run_skill. Pure Rust (getrandom
//! CSPRNG) → runs on all backends. Surfaces: chat + CLI. No standalone page —
//! key generation is non-deterministic, which doesn't fit the page's
//! live-recompute-on-input model.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_keypair_generator_core::{generate, Algorithm};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    #[serde(default = "default_algorithm")]
    algorithm: String,
}
fn default_algorithm() -> String {
    "ed25519".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None).param(
        Param::enumv("algorithm", ["x25519", "ed25519"])
            .default("ed25519")
            .describe(
                "Key algorithm: ed25519 (EdDSA signing keys — SSH, OpenPGP, JWT EdDSA, TLS) or x25519 (Curve25519 ECDH keys for key exchange / secure channels — Noise, WireGuard, HPKE, age). Default ed25519.",
            ),
    )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct KeypairGenerator;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/keypair-generator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Generate an X25519 or Ed25519 key pair",
    skill(
        description = "Generate a fresh X25519 or Ed25519 key pair entirely offline and return it in hex, base64, and PEM. Choose ed25519 for EdDSA signing keys (SSH, OpenPGP, JWT EdDSA, TLS) or x25519 for Curve25519 ECDH keys used to establish secure channels (Noise, WireGuard, HPKE, age). Returns the private key (PKCS#8 PEM plus the raw 32-byte scalar in base64 and hex) and the public key (SPKI PEM plus the raw 32-byte point in base64 and hex). Keys are generated locally with a cryptographic RNG; nothing is sent anywhere. Default algorithm is ed25519.",
        parameters = schema_json()
    ),
)]
impl KeypairGenerator {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "keypair-generator", |a: Args| {
            let algorithm = Algorithm::parse(&a.algorithm).map_err(SkillError::InvalidArgs)?;
            generate(algorithm).map_err(SkillError::InvalidArgs)
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
                    "algorithm": { "type": "string", "enum": ["x25519", "ed25519"], "default": "ed25519", "description": "Key algorithm: ed25519 (EdDSA signing keys — SSH, OpenPGP, JWT EdDSA, TLS) or x25519 (Curve25519 ECDH keys for key exchange / secure channels — Noise, WireGuard, HPKE, age). Default ed25519." }
                },
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
