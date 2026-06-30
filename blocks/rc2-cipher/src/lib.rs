//! gizza-ai/rc2-cipher — encrypt or decrypt data with the RC2 block cipher
//! (RFC 2268) at a configurable effective key length, in ECB or CBC mode.
//! Thin wrapper; chat schema single-sourced from descriptor(); handler delegates
//! to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_rc2_cipher_core::{decrypt, encrypt, Encoding, Mode};
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
    #[serde(default)]
    effective_key_bits: u32,
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
    effective_key_bits: u32,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe(
            "For encrypt: the plaintext (UTF-8). For decrypt: the ciphertext, encoded with `format`.",
        ))
        .param(
            Param::enumv("operation", ["encrypt", "decrypt"]).default("encrypt").describe("encrypt (default) or decrypt."),
        )
        .param(
            Param::enumv("cipher", ["cbc", "ecb"]).default("cbc").describe("RC2 mode of operation: cbc (default) or ecb."),
        )
        .param(Param::string("key").required().describe("The RC2 key (1-128 bytes), encoded with `format`."))
        .param(Param::string("iv").describe("The 8-byte IV, encoded with `format`. Required for cbc; omit for ecb."))
        .param(
            Param::integer("effective_key_bits").default(0).min(0.0).max(1024.0).describe(
                "RC2 effective key length T1 in bits (1-1024). 0 (default) = use the supplied key's full bit-length. Must match between encrypt and decrypt.",
            ),
        )
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
struct Rc2Cipher;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/rc2-cipher",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Encrypt or decrypt data with RC2 (RFC 2268) in ECB/CBC at a configurable effective key length",
    skill(
        description = "Encrypt or decrypt data with the RC2 block cipher (RFC 2268) in ECB or CBC mode, at a configurable effective key length (T1, in bits). key (1-128 bytes), iv (8 bytes, cbc only) and ciphertext are hex or base64 (set format; default base64); the plaintext is UTF-8. effective_key_bits 0 = use the key's full length; PKCS#7 padding. NOTE: RC2 is a legacy cipher (old PKCS#12 / S/MIME / Microsoft formats) and is NOT secure for new designs — use it only for interop or to decrypt legacy data; for real encryption use aes-cipher or text-encrypt.",
        parameters = schema_json()
    ),
)]
impl Rc2Cipher {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "rc2-cipher", |a: Args| {
            let mode = Mode::parse(&a.cipher).map_err(SkillError::InvalidArgs)?;
            let fmt = Encoding::parse(&a.format).map_err(SkillError::InvalidArgs)?;
            let output = match a.operation.trim().to_ascii_lowercase().as_str() {
                "encrypt" | "" => encrypt(&a.data, &a.key, &a.iv, a.effective_key_bits, mode, fmt),
                "decrypt" => decrypt(&a.data, &a.key, &a.iv, a.effective_key_bits, mode, fmt),
                other => Err(format!("unknown operation '{other}' (use encrypt or decrypt)")),
            }
            .map_err(SkillError::InvalidArgs)?;
            Ok(Resp {
                output,
                operation: a.operation,
                cipher: a.cipher,
                effective_key_bits: a.effective_key_bits,
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
                    "data": { "type": "string", "description": "For encrypt: the plaintext (UTF-8). For decrypt: the ciphertext, encoded with `format`." },
                    "operation": { "type": "string", "enum": ["encrypt", "decrypt"], "default": "encrypt", "description": "encrypt (default) or decrypt." },
                    "cipher": { "type": "string", "enum": ["cbc", "ecb"], "default": "cbc", "description": "RC2 mode of operation: cbc (default) or ecb." },
                    "key": { "type": "string", "description": "The RC2 key (1-128 bytes), encoded with `format`." },
                    "iv": { "type": "string", "description": "The 8-byte IV, encoded with `format`. Required for cbc; omit for ecb." },
                    "effective_key_bits": { "type": "integer", "minimum": 0, "maximum": 1024, "default": 0, "description": "RC2 effective key length T1 in bits (1-1024). 0 (default) = use the supplied key's full bit-length. Must match between encrypt and decrypt." },
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
