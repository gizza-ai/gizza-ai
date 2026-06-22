//! gizza-ai/ed25519-key-pair-generator — generate an Ed25519 key pair and return
//! the private (PKCS#8) and public (SPKI) keys in PEM, plus the raw 32-byte keys
//! in base64 and hex.
//!
//! Thin wrapper around the pure core; chat schema single-sourced from
//! descriptor(); handler delegates to run_skill. Pure Rust (getrandom CSPRNG) →
//! runs on all backends. Surfaces: chat + CLI + page (keygen is instant).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, SkillError, ToolDescriptor};
use gizza_ai_ed25519_key_pair_generator_core::generate;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize, Default)]
#[serde(default)]
struct Args {}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Ed25519KeyPairGenerator;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/ed25519-key-pair-generator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Generate an Ed25519 key pair (PEM + raw)",
    skill(
        description = "Generate a fresh Ed25519 key pair and return the private key (PKCS#8 PEM), the public key (SPKI PEM), and both raw 32-byte keys in base64 and hex. Keys are generated locally with a cryptographic RNG (getrandom). Ed25519 is a fast, modern EdDSA signature scheme used by SSH, OpenPGP, JWT (EdDSA), and TLS.",
        parameters = schema_json()
    )
)]
impl Ed25519KeyPairGenerator {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "ed25519-key-pair-generator", |_a: Args| {
            generate().map_err(SkillError::InvalidArgs)
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
                "properties": {},
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
