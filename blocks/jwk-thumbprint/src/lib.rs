//! gizza-ai/jwk-thumbprint — compute the RFC 7638 SHA-256 thumbprint of a JWK.
//! Chat schema single-sourced from descriptor(); handler delegates to run_skill.
//! Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_jwk_thumbprint_core::thumbprint;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    jwk: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None).param(
        Param::string("jwk")
            .required()
            .describe("The JSON Web Key (JWK) as JSON, e.g. {\"kty\":\"EC\",\"crv\":\"P-256\",\"x\":\"…\",\"y\":\"…\"}."),
    )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct JwkThumbprint;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/jwk-thumbprint",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Compute the RFC 7638 thumbprint of a JWK",
    skill(
        description = "Compute the RFC 7638 SHA-256 thumbprint of a JSON Web Key — the canonical key identifier commonly used as a JWK 'kid'. Hashes only the required members for the key type (RSA: e,kty,n; EC: crv,kty,x,y; OKP: crv,kty,x; oct: k,kty) in lexicographic order with no whitespace, then base64url-encodes the SHA-256 digest. Returns the thumbprint, the key type, and the exact canonical JSON that was hashed. Runs locally.",
        parameters = schema_json()
    ),
)]
impl JwkThumbprint {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "jwk-thumbprint", |a: Args| {
            thumbprint(&a.jwk).map_err(SkillError::InvalidArgs)
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
                    "jwk": { "type": "string", "description": "The JSON Web Key (JWK) as JSON, e.g. {\"kty\":\"EC\",\"crv\":\"P-256\",\"x\":\"…\",\"y\":\"…\"}." }
                },
                "required": ["jwk"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
