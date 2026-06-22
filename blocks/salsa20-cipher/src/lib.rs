//! gizza-ai/salsa20-cipher — encrypt or decrypt data with the Salsa20 stream
//! cipher (Salsa20/20) given a key and an 8-byte nonce. Thin wrapper; chat schema
//! single-sourced from descriptor(); handler delegates to run_skill. Pure → all
//! backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_salsa20_cipher_core::{decrypt, encrypt, Encoding, KeyFormat};
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default = "default_op")]
    operation: String,
    key: String,
    nonce: String,
    #[serde(default = "default_key_format")]
    key_format: String,
    #[serde(default)]
    counter: f64,
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
    counter: u64,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe(
            "For encrypt: the plaintext (UTF-8). For decrypt: the ciphertext, encoded with `format`.",
        ))
        .param(
            Param::enumv("operation", ["encrypt", "decrypt"]).default("encrypt").describe(
                "encrypt (default) or decrypt. Salsa20 is symmetric, so both apply the same keystream.",
            ),
        )
        .param(Param::string("key").required().describe(
            "The Salsa20 key. Must be 16 or 32 bytes: a 16- or 32-char UTF-8 passphrase when key_format=text (default), or a 32-/64-char hex (or base64) string when key_format=encoded.",
        ))
        .param(Param::string("nonce").required().describe(
            "The 8-byte nonce/IV. An 8-char UTF-8 string when key_format=text, or a 16-char hex (or base64) value when key_format=encoded. Must match on encrypt and decrypt, and must be unique per message for a given key.",
        ))
        .param(
            Param::enumv("key_format", ["text", "encoded"]).default("text").describe(
                "How to read the key and nonce: text = UTF-8 (default); encoded = hex/base64 per `format`.",
            ),
        )
        .param(
            Param::integer("counter").default(0).min(0.0).describe(
                "Initial 64-bit block counter (each block is 64 bytes). Default 0; set it to decrypt from an offset. Must match on encrypt and decrypt.",
            ),
        )
        .param(
            Param::enumv("format", ["hex", "base64"]).default("hex").describe(
                "Encoding for the ciphertext (and the key/nonce when key_format=encoded): hex (default) or base64. The plaintext is always UTF-8 text.",
            ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Salsa20Cipher;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/salsa20-cipher",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Encrypt or decrypt data with the Salsa20 stream cipher (key + 8-byte nonce)",
    skill(
        description = "Encrypt or decrypt data with the Salsa20 stream cipher (Salsa20/20, Daniel J. Bernstein's eSTREAM cipher). operation = encrypt (default) or decrypt — Salsa20 is symmetric. key must be 16 or 32 bytes and nonce exactly 8 bytes, read as UTF-8 (key_format=text, default) or decoded from hex/base64 (key_format=encoded). counter is the initial 64-bit block counter (default 0). The ciphertext (and an encoded key/nonce) use format = hex (default) or base64; the plaintext is UTF-8. The same key+nonce+counter must be used to decrypt, and a key+nonce pair must never be reused for two different messages. For password-based file encryption use aes-cipher or text-encrypt instead.",
        parameters = schema_json()
    ),
)]
impl Salsa20Cipher {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "salsa20-cipher", |a: Args| {
            let key_format = KeyFormat::parse(&a.key_format).map_err(SkillError::InvalidArgs)?;
            let fmt = Encoding::parse(&a.format).map_err(SkillError::InvalidArgs)?;
            if a.counter < 0.0 {
                return Err(SkillError::InvalidArgs("counter must be >= 0".into()));
            }
            let counter = a.counter as u64;
            let output = match a.operation.trim().to_ascii_lowercase().as_str() {
                "encrypt" | "" => encrypt(&a.data, &a.key, &a.nonce, key_format, counter, fmt),
                "decrypt" => decrypt(&a.data, &a.key, &a.nonce, key_format, counter, fmt),
                other => Err(format!("unknown operation '{other}' (use encrypt or decrypt)")),
            }
            .map_err(SkillError::InvalidArgs)?;
            Ok(Resp { output, operation: a.operation, counter })
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
                    "operation": { "type": "string", "enum": ["encrypt", "decrypt"], "default": "encrypt", "description": "encrypt (default) or decrypt. Salsa20 is symmetric, so both apply the same keystream." },
                    "key": { "type": "string", "description": "The Salsa20 key. Must be 16 or 32 bytes: a 16- or 32-char UTF-8 passphrase when key_format=text (default), or a 32-/64-char hex (or base64) string when key_format=encoded." },
                    "nonce": { "type": "string", "description": "The 8-byte nonce/IV. An 8-char UTF-8 string when key_format=text, or a 16-char hex (or base64) value when key_format=encoded. Must match on encrypt and decrypt, and must be unique per message for a given key." },
                    "key_format": { "type": "string", "enum": ["text", "encoded"], "default": "text", "description": "How to read the key and nonce: text = UTF-8 (default); encoded = hex/base64 per `format`." },
                    "counter": { "type": "integer", "default": 0, "minimum": 0, "description": "Initial 64-bit block counter (each block is 64 bytes). Default 0; set it to decrypt from an offset. Must match on encrypt and decrypt." },
                    "format": { "type": "string", "enum": ["hex", "base64"], "default": "hex", "description": "Encoding for the ciphertext (and the key/nonce when key_format=encoded): hex (default) or base64. The plaintext is always UTF-8 text." }
                },
                "required": ["data", "key", "nonce"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
