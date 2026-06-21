//! gizza-ai/aes-cipher — encrypt or decrypt text with AES (CBC/CTR/GCM/ECB,
//! 128/192/256-bit keys, hex/base64 I/O). Thin wrapper; chat schema single-sourced
//! from descriptor(); handler delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_aes_cipher_core::{decrypt, encrypt, Encoding, Mode};
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default = "default_op")]
    operation: String,
    #[serde(default = "default_cipher")]
    cipher: String,
    key: String,
    #[serde(default)]
    iv: String,
    #[serde(default = "default_format")]
    format: String,
}
fn default_op() -> String {
    "encrypt".to_string()
}
fn default_cipher() -> String {
    "gcm".to_string()
}
fn default_format() -> String {
    "base64".to_string()
}

#[derive(Serialize)]
struct Resp {
    output: String,
    operation: String,
    cipher: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe(
            "For encrypt: the plaintext. For decrypt: the ciphertext, encoded with `format`.",
        ))
        .param(
            Param::enumv("operation", ["encrypt", "decrypt"]).default("encrypt").describe(
                "encrypt (default) or decrypt.",
            ),
        )
        .param(
            Param::enumv("cipher", ["cbc", "ctr", "gcm", "ecb"]).default("gcm").describe(
                "AES mode of operation: gcm (default, authenticated), cbc, ctr, or ecb (insecure — avoid).",
            ),
        )
        .param(Param::string("key").required().describe(
            "The AES key, encoded with `format`. Its length sets the strength: 16 bytes = AES-128, 24 = AES-192, 32 = AES-256.",
        ))
        .param(Param::string("iv").describe(
            "The IV/nonce, encoded with `format`. Required for cbc/ctr (16 bytes) and gcm (12 bytes); omit for ecb.",
        ))
        .param(
            Param::enumv("format", ["base64", "hex"]).default("base64").describe(
                "Encoding for the key, iv, and ciphertext (base64 default, or hex). The plaintext is always UTF-8 text.",
            ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct AesCipher;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/aes-cipher",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Encrypt or decrypt text with AES (CBC/CTR/GCM/ECB)",
    skill(
        description = "Encrypt or decrypt text with AES using a raw key you provide. cipher = gcm (default, authenticated), cbc, ctr, or ecb; the key length selects AES-128/192/256 (16/24/32 bytes). key, iv and ciphertext are hex or base64 (set format; default base64); the plaintext is UTF-8. For gcm the 16-byte tag is appended to the ciphertext. cbc/ctr need a 16-byte iv, gcm a 12-byte nonce, ecb none. This is a low-level tool — for passphrase encryption with a safe random salt+nonce use text-encrypt instead.",
        parameters = schema_json()
    ),
)]
impl AesCipher {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "aes-cipher", |a: Args| {
            let mode = Mode::parse(&a.cipher).map_err(SkillError::InvalidArgs)?;
            let fmt = Encoding::parse(&a.format).map_err(SkillError::InvalidArgs)?;
            let output = match a.operation.trim().to_ascii_lowercase().as_str() {
                "encrypt" | "" => encrypt(&a.data, &a.key, &a.iv, mode, fmt),
                "decrypt" => decrypt(&a.data, &a.key, &a.iv, mode, fmt),
                other => Err(format!("unknown operation '{other}' (use encrypt or decrypt)")),
            }
            .map_err(SkillError::InvalidArgs)?;
            Ok(Resp { output, operation: a.operation, cipher: a.cipher })
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
                    "data": { "type": "string", "description": "For encrypt: the plaintext. For decrypt: the ciphertext, encoded with `format`." },
                    "operation": { "type": "string", "enum": ["encrypt", "decrypt"], "default": "encrypt", "description": "encrypt (default) or decrypt." },
                    "cipher": { "type": "string", "enum": ["cbc", "ctr", "gcm", "ecb"], "default": "gcm", "description": "AES mode of operation: gcm (default, authenticated), cbc, ctr, or ecb (insecure — avoid)." },
                    "key": { "type": "string", "description": "The AES key, encoded with `format`. Its length sets the strength: 16 bytes = AES-128, 24 = AES-192, 32 = AES-256." },
                    "iv": { "type": "string", "description": "The IV/nonce, encoded with `format`. Required for cbc/ctr (16 bytes) and gcm (12 bytes); omit for ecb." },
                    "format": { "type": "string", "enum": ["base64", "hex"], "default": "base64", "description": "Encoding for the key, iv, and ciphertext (base64 default, or hex). The plaintext is always UTF-8 text." }
                },
                "required": ["data", "key"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
