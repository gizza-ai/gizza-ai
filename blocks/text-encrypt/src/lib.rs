//! gizza-ai/text-encrypt — encrypt or decrypt text with a passphrase (AES-256-GCM).
//! Thin wrapper; chat schema single-sourced from descriptor(); handler delegates
//! to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_text_encrypt_core::{decrypt_text, encrypt_text};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    passphrase: String,
    #[serde(default = "default_mode")]
    mode: String,
}
fn default_mode() -> String {
    "encrypt".to_string()
}

#[derive(Serialize)]
struct Resp {
    result: String,
    mode: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The text to encrypt, or the base64 token to decrypt."),
        )
        .param(
            Param::string("passphrase")
                .required()
                .describe("The passphrase. The same passphrase is required to decrypt."),
        )
        .param(
            Param::enumv("mode", ["encrypt", "decrypt"]).default("encrypt").describe(
                "encrypt (default) turns text into a base64 token; decrypt turns a token back into text.",
            ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct TextEncrypt;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/text-encrypt",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Encrypt or decrypt text with a passphrase",
    skill(
        description = "Encrypt or decrypt text with a passphrase using AES-256-GCM (key derived via PBKDF2-HMAC-SHA256). mode=encrypt (default) turns text into a compact base64 token (a fresh random salt + nonce each time); mode=decrypt turns a token produced by this tool back into the original text. The same passphrase is required to decrypt; a wrong passphrase or tampered token fails cleanly. Runs locally — the text and passphrase never leave the device.",
        parameters = schema_json()
    ),
)]
impl TextEncrypt {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "text-encrypt", |a: Args| {
            let result = match a.mode.trim().to_ascii_lowercase().as_str() {
                "encrypt" | "" => encrypt_text(&a.text, &a.passphrase),
                "decrypt" => decrypt_text(&a.text, &a.passphrase),
                other => Err(format!("unknown mode '{other}' (use 'encrypt' or 'decrypt')")),
            }
            .map_err(SkillError::InvalidArgs)?;
            Ok(Resp { result, mode: a.mode })
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
                    "text": { "type": "string", "description": "The text to encrypt, or the base64 token to decrypt." },
                    "passphrase": { "type": "string", "description": "The passphrase. The same passphrase is required to decrypt." },
                    "mode": { "type": "string", "enum": ["encrypt", "decrypt"], "default": "encrypt", "description": "encrypt (default) turns text into a base64 token; decrypt turns a token back into text." }
                },
                "required": ["text", "passphrase"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
