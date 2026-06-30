//! gizza-ai/sm4-cipher — encrypt or decrypt data with SM4 (ECB or CBC).
//! Thin wrapper; chat schema single-sourced from descriptor(); handler delegates
//! to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_sm4_cipher_core::{decrypt, encrypt, Encoding, Mode};
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
    "cbc".to_string()
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
            Param::enumv("operation", ["encrypt", "decrypt"]).default("encrypt").describe("encrypt (default) or decrypt."),
        )
        .param(
            Param::enumv("cipher", ["cbc", "ecb"]).default("cbc").describe("SM4 mode of operation: cbc (default) or ecb."),
        )
        .param(Param::string("key").required().describe("The SM4 key — exactly 16 bytes (128 bits), encoded with `format`."))
        .param(Param::string("iv").describe("The 16-byte IV, encoded with `format`. Required for cbc; omit for ecb."))
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
struct Sm4Cipher;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/sm4-cipher",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Encrypt or decrypt data with SM4 (ECB/CBC)",
    skill(
        description = "Encrypt or decrypt data with the SM4 block cipher (the Chinese national standard GB/T 32907-2016) in ECB or CBC mode, using a 128-bit (16-byte) key and a 16-byte iv for cbc. key, iv and ciphertext are hex or base64 (set format; default base64); the plaintext is UTF-8. Both modes use PKCS#7 padding.",
        parameters = schema_json()
    ),
)]
impl Sm4Cipher {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "sm4-cipher", |a: Args| {
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
                    "cipher": { "type": "string", "enum": ["cbc", "ecb"], "default": "cbc", "description": "SM4 mode of operation: cbc (default) or ecb." },
                    "key": { "type": "string", "description": "The SM4 key — exactly 16 bytes (128 bits), encoded with `format`." },
                    "iv": { "type": "string", "description": "The 16-byte IV, encoded with `format`. Required for cbc; omit for ecb." },
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
