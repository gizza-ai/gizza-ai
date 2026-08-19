//! gizza-ai/age-encrypt — encrypt text into an ASCII-armored age file.
//! Pure Rust; runs in chat, CLI, and the generated browser page.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default = "default_mode")]
    mode: String,
    #[serde(default)]
    passphrase: String,
    #[serde(default)]
    recipients: String,
    #[serde(default = "default_work_factor")]
    work_factor: u8,
}

fn default_mode() -> String {
    "passphrase".to_string()
}

fn default_work_factor() -> u8 {
    gizza_ai_age_encrypt_core::DEFAULT_WORK_FACTOR
}

#[derive(Serialize)]
struct Resp {
    result: String,
    mode: String,
}

/// Single source for the chat schema, CLI parameters, and page form.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("Plaintext to encrypt into an ASCII-armored age file."),
        )
        .param(
            Param::enumv("mode", ["passphrase", "recipients"])
                .default("passphrase")
                .describe("passphrase encrypts with an scrypt-wrapped passphrase; recipients encrypts to one or more native age X25519 public keys."),
        )
        .param(
            Param::string("passphrase")
                .required()
                .describe("Passphrase for mode=passphrase. Leave blank when encrypting to recipients."),
        )
        .param(
            Param::string("recipients")
                .describe("One or more age X25519 public recipients starting with age1, separated by spaces, commas, semicolons, or new lines. Used only when mode=recipients."),
        )
        .param(
            Param::integer("work_factor")
                .default(default_work_factor())
                .min(gizza_ai_age_encrypt_core::MIN_WORK_FACTOR as f64)
                .max(gizza_ai_age_encrypt_core::MAX_WORK_FACTOR as f64)
                .describe("Scrypt log2 work factor for passphrase mode. Default 14; valid range 10-15 to stay within the wasm memory sandbox."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/age-encrypt",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Encrypt text to armored age ciphertext",
    skill(
        description = "Encrypt plaintext into an ASCII-armored age file. mode=passphrase (default) uses a passphrase with an explicit scrypt work factor capped for the wasm sandbox; mode=recipients encrypts to one or more native age X25519 public recipients that start with age1. The output begins with -----BEGIN AGE ENCRYPTED FILE----- and can be decrypted by compatible age clients with the same passphrase or matching identity. This tool encrypts small text locally; use the age CLI for large files, SSH recipients, identity generation, or decryption.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "age-encrypt", |a: Args| {
            let result = gizza_ai_age_encrypt_core::run(
                &a.text,
                &a.mode,
                &a.passphrase,
                &a.recipients,
                a.work_factor,
            )
            .map_err(SkillError::InvalidArgs)?;
            Ok(Resp {
                result,
                mode: gizza_ai_age_encrypt_core::Mode::parse(&a.mode)
                    .map_err(SkillError::InvalidArgs)?
                    .as_str()
                    .to_string(),
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
                    "text": { "type": "string", "description": "Plaintext to encrypt into an ASCII-armored age file." },
                    "mode": { "type": "string", "enum": ["passphrase", "recipients"], "default": "passphrase", "description": "passphrase encrypts with an scrypt-wrapped passphrase; recipients encrypts to one or more native age X25519 public keys." },
                    "passphrase": { "type": "string", "description": "Passphrase for mode=passphrase. Leave blank when encrypting to recipients." },
                    "recipients": { "type": "string", "description": "One or more age X25519 public recipients starting with age1, separated by spaces, commas, semicolons, or new lines. Used only when mode=recipients." },
                    "work_factor": { "type": "integer", "default": 14, "minimum": 10, "maximum": 15, "description": "Scrypt log2 work factor for passphrase mode. Default 14; valid range 10-15 to stay within the wasm memory sandbox." }
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
