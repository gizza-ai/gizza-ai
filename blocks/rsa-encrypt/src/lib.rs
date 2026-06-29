//! gizza-ai/rsa-encrypt — encrypt a small payload to an RSA public key (OAEP or
//! PKCS#1 v1.5) and return base64 ciphertext. Thin wrapper; chat schema
//! single-sourced from descriptor(); handler delegates to run_skill. Pure → all
//! backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_rsa_encrypt_core::{encrypt, Hash, Padding};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    message: String,
    public_key: String,
    #[serde(default = "default_padding")]
    padding: String,
    #[serde(default = "default_hash")]
    hash: String,
}

fn default_padding() -> String {
    "oaep".to_string()
}
fn default_hash() -> String {
    "sha256".to_string()
}

#[derive(Serialize)]
struct Resp {
    ciphertext: String,
    padding: String,
    hash: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("message")
                .required()
                .describe("The plaintext message to encrypt. Must fit in one RSA block (e.g. up to ~190 bytes for a 2048-bit key with OAEP-SHA256)."),
        )
        .param(Param::string("public_key").required().describe(
            "The recipient's RSA public key, PEM-encoded (SPKI '-----BEGIN PUBLIC KEY-----' or PKCS#1 '-----BEGIN RSA PUBLIC KEY-----').",
        ))
        .param(
            Param::enumv("padding", ["oaep", "pkcs1v15"]).default("oaep").describe(
                "Padding scheme: oaep (default, RSAES-OAEP, recommended) or pkcs1v15 (legacy RSAES-PKCS1-v1_5).",
            ),
        )
        .param(
            Param::enumv("hash", ["sha256", "sha384", "sha512"]).default("sha256").describe(
                "OAEP digest (MGF1 + label hash); used only when padding=oaep (default sha256). Ignored for pkcs1v15.",
            ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct RsaEncrypt;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/rsa-encrypt",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Encrypt a message to an RSA public key",
    skill(
        description = "Encrypt a small payload to an RSA public key and return base64 ciphertext. padding=oaep (default, RSAES-OAEP, recommended) or pkcs1v15 (legacy RSAES-PKCS1-v1_5); hash=sha256 (default), sha384, or sha512 selects the OAEP digest (ignored for pkcs1v15). The public key is PEM-encoded (SPKI or PKCS#1). Only the holder of the matching private key can decrypt. The message must fit in one RSA block (≈190 bytes for a 2048-bit key with OAEP-SHA256) — for larger data, encrypt a symmetric key instead. Runs locally — nothing leaves the device.",
        parameters = schema_json()
    ),
)]
impl RsaEncrypt {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "rsa-encrypt", |a: Args| {
            let padding = Padding::parse(&a.padding).map_err(SkillError::InvalidArgs)?;
            let hash = Hash::parse(&a.hash).map_err(SkillError::InvalidArgs)?;
            let ciphertext =
                encrypt(&a.message, &a.public_key, padding, hash).map_err(SkillError::InvalidArgs)?;
            Ok(Resp {
                ciphertext,
                padding: a.padding,
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
                    "message": { "type": "string", "description": "The plaintext message to encrypt. Must fit in one RSA block (e.g. up to ~190 bytes for a 2048-bit key with OAEP-SHA256)." },
                    "public_key": { "type": "string", "description": "The recipient's RSA public key, PEM-encoded (SPKI '-----BEGIN PUBLIC KEY-----' or PKCS#1 '-----BEGIN RSA PUBLIC KEY-----')." },
                    "padding": { "type": "string", "enum": ["oaep", "pkcs1v15"], "default": "oaep", "description": "Padding scheme: oaep (default, RSAES-OAEP, recommended) or pkcs1v15 (legacy RSAES-PKCS1-v1_5)." },
                    "hash": { "type": "string", "enum": ["sha256", "sha384", "sha512"], "default": "sha256", "description": "OAEP digest (MGF1 + label hash); used only when padding=oaep (default sha256). Ignored for pkcs1v15." }
                },
                "required": ["message", "public_key"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
