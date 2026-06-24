//! gizza-ai/rsa-verify — verify an RSA signature (PKCS#1 v1.5 or PSS) over a
//! message against an RSA public key, returning whether it is valid. Thin wrapper;
//! chat schema single-sourced from descriptor(); handler delegates to run_skill.
//! Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_rsa_verify_core::{verify, Hash, Scheme};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    message: String,
    signature: String,
    public_key: String,
    #[serde(default = "default_scheme")]
    scheme: String,
    #[serde(default = "default_hash")]
    hash: String,
}

fn default_scheme() -> String {
    "pkcs1v15".to_string()
}
fn default_hash() -> String {
    "sha256".to_string()
}

#[derive(Serialize)]
struct Resp {
    valid: bool,
    scheme: String,
    hash: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("message")
                .required()
                .describe("The original message that was signed."),
        )
        .param(
            Param::string("signature")
                .required()
                .describe("The signature to verify, base64-encoded."),
        )
        .param(Param::string("public_key").required().describe(
            "The signer's RSA public key, PEM-encoded (SPKI '-----BEGIN PUBLIC KEY-----' or PKCS#1 '-----BEGIN RSA PUBLIC KEY-----').",
        ))
        .param(
            Param::enumv("scheme", ["pkcs1v15", "pss"]).default("pkcs1v15").describe(
                "Signature scheme the signature was produced with: pkcs1v15 (default) or pss.",
            ),
        )
        .param(
            Param::enumv("hash", ["sha256", "sha384", "sha512"]).default("sha256").describe(
                "Digest algorithm the message was hashed with before signing (default sha256).",
            ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct RsaVerify;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/rsa-verify",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Verify an RSA signature against a public key",
    skill(
        description = "Verify an RSA signature over a message against an RSA public key, returning whether it is valid (true/false). scheme=pkcs1v15 (default, RSASSA-PKCS1-v1_5) or pss (RSASSA-PSS); hash=sha256 (default), sha384, or sha512 — these must match how the signature was produced. The signature is base64-encoded; the public key is PEM-encoded (SPKI or PKCS#1). Runs locally — the inputs never leave the device.",
        parameters = schema_json()
    ),
)]
impl RsaVerify {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "rsa-verify", |a: Args| {
            let scheme = Scheme::parse(&a.scheme).map_err(SkillError::InvalidArgs)?;
            let hash = Hash::parse(&a.hash).map_err(SkillError::InvalidArgs)?;
            let valid = verify(&a.message, &a.signature, &a.public_key, scheme, hash)
                .map_err(SkillError::InvalidArgs)?;
            Ok(Resp {
                valid,
                scheme: a.scheme,
                hash: a.hash,
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
                    "message": { "type": "string", "description": "The original message that was signed." },
                    "signature": { "type": "string", "description": "The signature to verify, base64-encoded." },
                    "public_key": { "type": "string", "description": "The signer's RSA public key, PEM-encoded (SPKI '-----BEGIN PUBLIC KEY-----' or PKCS#1 '-----BEGIN RSA PUBLIC KEY-----')." },
                    "scheme": { "type": "string", "enum": ["pkcs1v15", "pss"], "default": "pkcs1v15", "description": "Signature scheme the signature was produced with: pkcs1v15 (default) or pss." },
                    "hash": { "type": "string", "enum": ["sha256", "sha384", "sha512"], "default": "sha256", "description": "Digest algorithm the message was hashed with before signing (default sha256)." }
                },
                "required": ["message", "signature", "public_key"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
