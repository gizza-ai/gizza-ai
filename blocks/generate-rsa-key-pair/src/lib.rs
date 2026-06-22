//! gizza-ai/generate-rsa-key-pair — generate an RSA key pair (2048/3072/4096) and
//! return the private (PKCS#8) and public (SPKI) keys in PEM.
//!
//! Thin wrapper around the pure core; chat schema single-sourced from
//! descriptor(); handler delegates to run_skill. Pure Rust (getrandom CSPRNG) →
//! runs on all backends. Surfaces: chat + CLI. No standalone page — RSA key
//! generation is heavy and non-deterministic, which doesn't fit the page's
//! live-recompute-on-input model.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_generate_rsa_key_pair_core::generate;
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    #[serde(default = "default_key_size")]
    key_size: String,
}
fn default_key_size() -> String {
    "2048".to_string()
}

#[derive(Serialize)]
struct Resp {
    private_pem: String,
    public_pem: String,
    bits: usize,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None).param(
        Param::enumv("key_size", ["2048", "3072", "4096"])
            .default("2048")
            .describe("RSA key size in bits. Larger is more secure but slower to generate. Default 2048."),
    )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct GenerateRsaKeyPair;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/generate-rsa-key-pair",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Generate an RSA key pair (PEM)",
    skill(
        description = "Generate a fresh RSA key pair of the chosen size (2048, 3072, or 4096 bits) and return the private key (PKCS#8 PEM) and public key (SPKI PEM). Keys are generated locally with a cryptographic RNG. Default key size is 2048.",
        parameters = schema_json()
    )
)]
impl GenerateRsaKeyPair {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "generate-rsa-key-pair", |a: Args| {
            let bits: usize = a
                .key_size
                .trim()
                .parse()
                .map_err(|_| SkillError::InvalidArgs(format!("invalid key_size '{}'", a.key_size)))?;
            let kp = generate(bits).map_err(SkillError::InvalidArgs)?;
            Ok(Resp { private_pem: kp.private_pem, public_pem: kp.public_pem, bits: kp.bits })
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
                    "key_size": { "type": "string", "enum": ["2048", "3072", "4096"], "default": "2048", "description": "RSA key size in bits. Larger is more secure but slower to generate. Default 2048." }
                },
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
