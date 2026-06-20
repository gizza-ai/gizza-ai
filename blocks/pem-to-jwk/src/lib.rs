//! gizza-ai/pem-to-jwk — convert a PEM-encoded RSA/EC key into its JWK
//! representation. Thin wrapper; chat schema single-sourced from descriptor();
//! handler delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    pem: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None).param(
        Param::string("pem")
            .required()
            .describe("The PEM-encoded key, including the -----BEGIN ...----- / -----END ...----- lines. RSA (PKCS#1/PKCS#8/SPKI) or EC over P-256, P-384, P-521 (SEC1/PKCS#8/SPKI), public or private."),
    )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct PemToJwk;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/pem-to-jwk",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert a PEM key to a JSON Web Key (JWK)",
    skill(
        description = "Convert a PEM-encoded cryptographic key into the equivalent JSON Web Key (JWK, RFC 7517/7518). Supports RSA (PKCS#1, PKCS#8, or SPKI) and EC over the NIST curves P-256, P-384 and P-521 (SEC1, PKCS#8, or SPKI). A public key yields a public JWK ({kty, n, e} or {kty, crv, x, y}); a private key yields a private JWK with the private components (RSA d/p/q/dp/dq/qi, or EC d). All binary members are base64url-encoded per spec. Runs locally — the key never leaves the device.",
        parameters = schema_json()
    )
)]
impl PemToJwk {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "pem-to-jwk", |a: Args| {
            gizza_ai_pem_to_jwk_core::run(&a.pem).map_err(SkillError::InvalidArgs)
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
                    "pem": { "type": "string", "description": "The PEM-encoded key, including the -----BEGIN ...----- / -----END ...----- lines. RSA (PKCS#1/PKCS#8/SPKI) or EC over P-256, P-384, P-521 (SEC1/PKCS#8/SPKI), public or private." }
                },
                "required": ["pem"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
