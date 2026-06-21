//! gizza-ai/rsa-sign — sign a message with an RSA private key (PKCS#1 v1.5 or PSS)
//! and return a base64 signature. Thin wrapper; chat schema single-sourced from
//! descriptor(); handler delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_rsa_sign_core::{sign, Hash, Scheme};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    message: String,
    private_key: String,
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
    signature: String,
    scheme: String,
    hash: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("message")
                .required()
                .describe("The message to sign."),
        )
        .param(Param::string("private_key").required().describe(
            "Your RSA private key, PEM-encoded (PKCS#8 '-----BEGIN PRIVATE KEY-----' or PKCS#1 '-----BEGIN RSA PRIVATE KEY-----').",
        ))
        .param(
            Param::enumv("scheme", ["pkcs1v15", "pss"]).default("pkcs1v15").describe(
                "Signature scheme: pkcs1v15 (default, deterministic) or pss (randomized).",
            ),
        )
        .param(
            Param::enumv("hash", ["sha256", "sha384", "sha512"]).default("sha256").describe(
                "Digest algorithm to hash the message with before signing (default sha256).",
            ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct RsaSign;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/rsa-sign",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Sign a message with an RSA private key",
    skill(
        description = "Sign a message with an RSA private key and return a base64-encoded signature. scheme=pkcs1v15 (default, deterministic RSASSA-PKCS1-v1_5) or pss (randomized RSASSA-PSS); hash=sha256 (default), sha384, or sha512. The private key is PEM-encoded (PKCS#8 or PKCS#1). The signature verifies with the matching public key. Runs locally — the private key and message never leave the device.",
        parameters = schema_json()
    ),
)]
impl RsaSign {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "rsa-sign", |a: Args| {
            let scheme = Scheme::parse(&a.scheme).map_err(SkillError::InvalidArgs)?;
            let hash = Hash::parse(&a.hash).map_err(SkillError::InvalidArgs)?;
            let signature =
                sign(&a.message, &a.private_key, scheme, hash).map_err(SkillError::InvalidArgs)?;
            Ok(Resp {
                signature,
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
                    "message": { "type": "string", "description": "The message to sign." },
                    "private_key": { "type": "string", "description": "Your RSA private key, PEM-encoded (PKCS#8 '-----BEGIN PRIVATE KEY-----' or PKCS#1 '-----BEGIN RSA PRIVATE KEY-----')." },
                    "scheme": { "type": "string", "enum": ["pkcs1v15", "pss"], "default": "pkcs1v15", "description": "Signature scheme: pkcs1v15 (default, deterministic) or pss (randomized)." },
                    "hash": { "type": "string", "enum": ["sha256", "sha384", "sha512"], "default": "sha256", "description": "Digest algorithm to hash the message with before signing (default sha256)." }
                },
                "required": ["message", "private_key"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
