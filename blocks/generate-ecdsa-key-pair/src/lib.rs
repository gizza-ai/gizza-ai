//! gizza-ai/generate-ecdsa-key-pair — generate an ECDSA key pair on a NIST prime
//! curve (P-256/P-384/P-521) and return the private (PKCS#8) and public (SPKI)
//! keys in PEM, optionally also as JWK (RFC 7517).
//!
//! Thin wrapper around the pure core; chat schema single-sourced from
//! descriptor(); handler delegates to run_skill. Pure Rust (getrandom CSPRNG) →
//! runs on all backends. Surfaces: chat + CLI. No standalone page — key
//! generation is non-deterministic, which doesn't fit the page's
//! live-recompute-on-input model.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_generate_ecdsa_key_pair_core::{generate, Curve};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    #[serde(default = "default_curve")]
    curve: String,
    #[serde(default)]
    jwk: bool,
}
fn default_curve() -> String {
    "p256".to_string()
}

#[derive(Serialize)]
struct Resp {
    private_pem: String,
    public_pem: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    private_jwk: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    public_jwk: Option<String>,
    curve: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::enumv("curve", ["p256", "p384", "p521"])
                .default("p256")
                .describe("NIST prime curve: p256 (secp256r1), p384 (secp384r1), or p521 (secp521r1). Default p256."),
        )
        .param(
            Param::boolean("jwk")
                .default(false)
                .describe("Also output the keys as JSON Web Keys (RFC 7517), in addition to PEM. Default false."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct GenerateEcdsaKeyPair;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/generate-ecdsa-key-pair",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Generate an ECDSA key pair (PEM/JWK)",
    skill(
        description = "Generate a fresh ECDSA key pair on a NIST prime curve (p256, p384, or p521) and return the private key (PKCS#8 PEM) and public key (SPKI PEM). Optionally also output JSON Web Keys (RFC 7517) by setting jwk=true. Keys are generated locally with a cryptographic RNG. Default curve is p256.",
        parameters = schema_json()
    )
)]
impl GenerateEcdsaKeyPair {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "generate-ecdsa-key-pair", |a: Args| {
            let curve = Curve::parse(&a.curve).map_err(SkillError::InvalidArgs)?;
            let kp = generate(curve, a.jwk).map_err(SkillError::InvalidArgs)?;
            Ok(Resp {
                private_pem: kp.private_pem,
                public_pem: kp.public_pem,
                private_jwk: kp.private_jwk,
                public_jwk: kp.public_jwk,
                curve: kp.curve,
            })
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
                    "curve": { "type": "string", "enum": ["p256", "p384", "p521"], "default": "p256", "description": "NIST prime curve: p256 (secp256r1), p384 (secp384r1), or p521 (secp521r1). Default p256." },
                    "jwk": { "type": "boolean", "default": false, "description": "Also output the keys as JSON Web Keys (RFC 7517), in addition to PEM. Default false." }
                },
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
