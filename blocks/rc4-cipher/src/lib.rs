//! gizza-ai/rc4-cipher — encrypt or decrypt data with the RC4 stream cipher,
//! with an optional drop-N (RC4-drop) to skip the weak initial keystream bytes.
//! Thin wrapper; chat schema single-sourced from descriptor(); handler delegates
//! to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_rc4_cipher_core::{decrypt, encrypt, Encoding, KeyFormat};
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default = "default_op")]
    operation: String,
    key: String,
    #[serde(default = "default_key_format")]
    key_format: String,
    #[serde(default)]
    drop: f64,
    #[serde(default = "default_format")]
    format: String,
}
fn default_op() -> String {
    "encrypt".to_string()
}
fn default_key_format() -> String {
    "text".to_string()
}
fn default_format() -> String {
    "hex".to_string()
}

#[derive(Serialize)]
struct Resp {
    output: String,
    operation: String,
    drop: u64,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe(
            "For encrypt: the plaintext (UTF-8). For decrypt: the ciphertext, encoded with `format`.",
        ))
        .param(
            Param::enumv("operation", ["encrypt", "decrypt"]).default("encrypt").describe(
                "encrypt (default) or decrypt. RC4 is symmetric, so both apply the same keystream.",
            ),
        )
        .param(Param::string("key").required().describe(
            "The RC4 key (1–256 bytes). Read as a UTF-8 passphrase when key_format=text (default), or decoded from `format` when key_format=encoded.",
        ))
        .param(
            Param::enumv("key_format", ["text", "encoded"]).default("text").describe(
                "How to read the key: text = UTF-8 passphrase (default); encoded = hex/base64 per `format`.",
            ),
        )
        .param(
            Param::integer("drop").default(0).min(0.0).describe(
                "RC4-drop[n]: discard the first n keystream bytes before use (e.g. 768/3072) to skip RC4's biased prefix. Must match on encrypt and decrypt. Default 0 (plain RC4).",
            ),
        )
        .param(
            Param::enumv("format", ["hex", "base64"]).default("hex").describe(
                "Encoding for the ciphertext (and the key when key_format=encoded): hex (default) or base64. The plaintext is always UTF-8 text.",
            ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Rc4Cipher;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/rc4-cipher",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Encrypt or decrypt data with the RC4 stream cipher (optional drop-N)",
    skill(
        description = "Encrypt or decrypt data with the RC4 stream cipher, with an optional drop-N (RC4-drop[n]) to skip the weak initial keystream bytes. operation = encrypt (default) or decrypt — RC4 is symmetric. key is 1–256 bytes, read as a UTF-8 passphrase (key_format=text, default) or decoded from hex/base64 (key_format=encoded). drop discards the first n keystream bytes (e.g. 768/3072); it must match on both sides. The ciphertext (and an encoded key) use format = hex (default) or base64; the plaintext is UTF-8. WARNING: RC4 is cryptographically broken — use this for interop, CTFs, and legacy systems only; for real encryption use aes-cipher or text-encrypt.",
        parameters = schema_json()
    ),
)]
impl Rc4Cipher {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "rc4-cipher", |a: Args| {
            let key_format = KeyFormat::parse(&a.key_format).map_err(SkillError::InvalidArgs)?;
            let fmt = Encoding::parse(&a.format).map_err(SkillError::InvalidArgs)?;
            if a.drop < 0.0 {
                return Err(SkillError::InvalidArgs("drop must be >= 0".into()));
            }
            let drop = a.drop as usize;
            let output = match a.operation.trim().to_ascii_lowercase().as_str() {
                "encrypt" | "" => encrypt(&a.data, &a.key, key_format, drop, fmt),
                "decrypt" => decrypt(&a.data, &a.key, key_format, drop, fmt),
                other => Err(format!("unknown operation '{other}' (use encrypt or decrypt)")),
            }
            .map_err(SkillError::InvalidArgs)?;
            Ok(Resp { output, operation: a.operation, drop: drop as u64 })
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
                    "operation": { "type": "string", "enum": ["encrypt", "decrypt"], "default": "encrypt", "description": "encrypt (default) or decrypt. RC4 is symmetric, so both apply the same keystream." },
                    "key": { "type": "string", "description": "The RC4 key (1–256 bytes). Read as a UTF-8 passphrase when key_format=text (default), or decoded from `format` when key_format=encoded." },
                    "key_format": { "type": "string", "enum": ["text", "encoded"], "default": "text", "description": "How to read the key: text = UTF-8 passphrase (default); encoded = hex/base64 per `format`." },
                    "drop": { "type": "integer", "default": 0, "minimum": 0, "description": "RC4-drop[n]: discard the first n keystream bytes before use (e.g. 768/3072) to skip RC4's biased prefix. Must match on encrypt and decrypt. Default 0 (plain RC4)." },
                    "format": { "type": "string", "enum": ["hex", "base64"], "default": "hex", "description": "Encoding for the ciphertext (and the key when key_format=encoded): hex (default) or base64. The plaintext is always UTF-8 text." }
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
