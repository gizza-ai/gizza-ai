//! gizza-ai/rabbit-cipher — encrypt or decrypt data with the Rabbit stream
//! cipher (RFC 4503 / eSTREAM): a 128-bit key + optional 64-bit IV. Thin wrapper;
//! chat schema single-sourced from descriptor(); handler delegates to run_skill.
//! Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_rabbit_cipher_core::{decrypt, encrypt, Encoding, KeyFormat};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default = "default_op")]
    operation: String,
    key: String,
    #[serde(default)]
    iv: String,
    #[serde(default = "default_key_format")]
    key_format: String,
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
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe(
            "For encrypt: the plaintext (UTF-8). For decrypt: the ciphertext, encoded with `format`.",
        ))
        .param(
            Param::enumv("operation", ["encrypt", "decrypt"]).default("encrypt").describe(
                "encrypt (default) or decrypt. Rabbit is symmetric, so both apply the same keystream.",
            ),
        )
        .param(Param::string("key").required().describe(
            "The Rabbit key. Must resolve to exactly 16 bytes (128 bits): a 16-character UTF-8 passphrase when key_format=text (default), or 32 hex chars / 24 base64 chars when key_format=encoded.",
        ))
        .param(Param::string("iv").describe(
            "Optional initialization vector. Must resolve to exactly 8 bytes (64 bits) when given: 8 UTF-8 chars (key_format=text) or 16 hex chars / 12 base64 chars (key_format=encoded). Leave empty for no IV. Must match on encrypt and decrypt.",
        ))
        .param(
            Param::enumv("key_format", ["text", "encoded"]).default("text").describe(
                "How to read the key and IV: text = UTF-8 bytes (default); encoded = hex/base64 per `format`.",
            ),
        )
        .param(
            Param::enumv("format", ["hex", "base64"]).default("hex").describe(
                "Encoding for the ciphertext (and the key/IV when key_format=encoded): hex (default) or base64. The plaintext is always UTF-8 text.",
            ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct RabbitCipher;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/rabbit-cipher",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Encrypt or decrypt data with the Rabbit stream cipher (128-bit key, optional 64-bit IV)",
    skill(
        description = "Encrypt or decrypt data with the Rabbit stream cipher (RFC 4503 / eSTREAM), a fast 128-bit-key, 64-bit-IV stream cipher. operation = encrypt (default) or decrypt — Rabbit is symmetric, so both apply the same keystream. key must resolve to exactly 16 bytes: a 16-char UTF-8 passphrase (key_format=text, default) or hex/base64 (key_format=encoded). iv is optional and, when given, must resolve to exactly 8 bytes; it must match on both sides. The ciphertext (and an encoded key/IV) use format = hex (default) or base64; the plaintext is UTF-8. Keys and IVs are read most-significant-byte first, matching the RFC 4503 test vectors.",
        parameters = schema_json()
    ),
)]
impl RabbitCipher {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "rabbit-cipher", |a: Args| {
            let key_format = KeyFormat::parse(&a.key_format).map_err(SkillError::InvalidArgs)?;
            let fmt = Encoding::parse(&a.format).map_err(SkillError::InvalidArgs)?;
            let output = match a.operation.trim().to_ascii_lowercase().as_str() {
                "encrypt" | "" => encrypt(&a.data, &a.key, &a.iv, key_format, fmt),
                "decrypt" => decrypt(&a.data, &a.key, &a.iv, key_format, fmt),
                other => Err(format!("unknown operation '{other}' (use encrypt or decrypt)")),
            }
            .map_err(SkillError::InvalidArgs)?;
            Ok(Resp { output, operation: a.operation })
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
                    "operation": { "type": "string", "enum": ["encrypt", "decrypt"], "default": "encrypt", "description": "encrypt (default) or decrypt. Rabbit is symmetric, so both apply the same keystream." },
                    "key": { "type": "string", "description": "The Rabbit key. Must resolve to exactly 16 bytes (128 bits): a 16-character UTF-8 passphrase when key_format=text (default), or 32 hex chars / 24 base64 chars when key_format=encoded." },
                    "iv": { "type": "string", "description": "Optional initialization vector. Must resolve to exactly 8 bytes (64 bits) when given: 8 UTF-8 chars (key_format=text) or 16 hex chars / 12 base64 chars (key_format=encoded). Leave empty for no IV. Must match on encrypt and decrypt." },
                    "key_format": { "type": "string", "enum": ["text", "encoded"], "default": "text", "description": "How to read the key and IV: text = UTF-8 bytes (default); encoded = hex/base64 per `format`." },
                    "format": { "type": "string", "enum": ["hex", "base64"], "default": "hex", "description": "Encoding for the ciphertext (and the key/IV when key_format=encoded): hex (default) or base64. The plaintext is always UTF-8 text." }
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
