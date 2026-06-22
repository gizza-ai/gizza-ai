//! gizza-ai/ciphersaber2 — encrypt or decrypt data with CipherSaber-2 (RC4 with
//! a random 10-byte IV prepended to the key and the key-setup loop repeated
//! `rounds` times). Thin wrapper; chat schema single-sourced from descriptor();
//! handler delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_ciphersaber2_core::{decrypt, encrypt, Encoding, KeyFormat, IV_LEN};
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
    #[serde(default = "default_rounds")]
    rounds: f64,
    #[serde(default)]
    iv: String,
    #[serde(default = "default_format")]
    format: String,
}
fn default_op() -> String {
    "encrypt".to_string()
}
fn default_key_format() -> String {
    "text".to_string()
}
fn default_rounds() -> f64 {
    20.0
}
fn default_format() -> String {
    "hex".to_string()
}

#[derive(Serialize)]
struct Resp {
    output: String,
    operation: String,
    rounds: u64,
}

/// Parse an optional explicit IV: empty => None (random IV is generated).
/// Otherwise it must decode (hex per default, or `format`) to exactly 10 bytes.
fn parse_iv(s: &str, fmt: Encoding) -> Result<Option<[u8; IV_LEN]>, String> {
    let s = s.trim();
    if s.is_empty() {
        return Ok(None);
    }
    let bytes = fmt.decode(s)?;
    if bytes.len() != IV_LEN {
        return Err(format!("iv must be exactly {IV_LEN} bytes, got {}", bytes.len()));
    }
    let mut iv = [0u8; IV_LEN];
    iv.copy_from_slice(&bytes);
    Ok(Some(iv))
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe(
            "For encrypt: the plaintext (UTF-8). For decrypt: the ciphertext (encoded `IV || RC4(plaintext)`) per `format`.",
        ))
        .param(
            Param::enumv("operation", ["encrypt", "decrypt"]).default("encrypt").describe(
                "encrypt (default) or decrypt. CipherSaber is symmetric; both apply the same RC4 keystream.",
            ),
        )
        .param(Param::string("key").required().describe(
            "The key (a passphrase, or encoded bytes). Read as a UTF-8 passphrase when key_format=text (default), or decoded from `format` when key_format=encoded.",
        ))
        .param(
            Param::enumv("key_format", ["text", "encoded"]).default("text").describe(
                "How to read the key: text = UTF-8 passphrase (default); encoded = hex/base64 per `format`.",
            ),
        )
        .param(
            Param::integer("rounds").default(20).min(1.0).describe(
                "CipherSaber-2 key-setup iterations: the number of times RC4's KSA mixing loop runs (the spec recommends 20). Must match on encrypt and decrypt; 1 = CipherSaber-1.",
            ),
        )
        .param(
            Param::string("iv").describe(
                "Optional explicit 10-byte IV, encoded per `format` (hex/base64). Leave empty to generate a fresh random IV (recommended). Used for deterministic/interop output. Ignored on decrypt (the IV is read from the ciphertext).",
            ),
        )
        .param(
            Param::enumv("format", ["hex", "base64"]).default("hex").describe(
                "Encoding for the ciphertext (and the key/iv when encoded): hex (default) or base64. The plaintext is always UTF-8 text.",
            ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct CipherSaber2;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/ciphersaber2",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Encrypt or decrypt data with CipherSaber-2 (RC4 + random IV, repeated key setup)",
    skill(
        description = "Encrypt or decrypt data with CipherSaber-2, a minimalist symmetric cipher built on RC4 (ARCFOUR). Each message uses a session key = your key + a fresh random 10-byte IV, and CipherSaber-2 repeats RC4's key-scheduling loop `rounds` times (the spec recommends 20) to resist the FMS attack. operation = encrypt (default) or decrypt — it is symmetric. key is a passphrase (key_format=text, default) or decoded bytes (key_format=encoded, hex/base64 per format). The IV is prepended to the ciphertext in the clear; on encrypt a random IV is generated unless you pass an explicit 10-byte `iv`. rounds must match on both sides. format = hex (default) or base64. WARNING: RC4 is cryptographically broken — use this for interop, CTFs and learning only; for real encryption use aes-cipher, text-encrypt or encrypt-file.",
        parameters = schema_json()
    ),
)]
impl CipherSaber2 {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "ciphersaber2", |a: Args| {
            let key_format = KeyFormat::parse(&a.key_format).map_err(SkillError::InvalidArgs)?;
            let fmt = Encoding::parse(&a.format).map_err(SkillError::InvalidArgs)?;
            if a.rounds < 1.0 {
                return Err(SkillError::InvalidArgs("rounds must be >= 1".into()));
            }
            let rounds = a.rounds as u32;
            let output = match a.operation.trim().to_ascii_lowercase().as_str() {
                "encrypt" | "" => {
                    let iv = parse_iv(&a.iv, fmt).map_err(SkillError::InvalidArgs)?;
                    encrypt(&a.data, &a.key, key_format, rounds, iv, fmt)
                }
                "decrypt" => decrypt(&a.data, &a.key, key_format, rounds, fmt),
                other => Err(format!("unknown operation '{other}' (use encrypt or decrypt)")),
            }
            .map_err(SkillError::InvalidArgs)?;
            Ok(Resp { output, operation: a.operation, rounds: rounds as u64 })
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
                    "data": { "type": "string", "description": "For encrypt: the plaintext (UTF-8). For decrypt: the ciphertext (encoded `IV || RC4(plaintext)`) per `format`." },
                    "operation": { "type": "string", "enum": ["encrypt", "decrypt"], "default": "encrypt", "description": "encrypt (default) or decrypt. CipherSaber is symmetric; both apply the same RC4 keystream." },
                    "key": { "type": "string", "description": "The key (a passphrase, or encoded bytes). Read as a UTF-8 passphrase when key_format=text (default), or decoded from `format` when key_format=encoded." },
                    "key_format": { "type": "string", "enum": ["text", "encoded"], "default": "text", "description": "How to read the key: text = UTF-8 passphrase (default); encoded = hex/base64 per `format`." },
                    "rounds": { "type": "integer", "default": 20, "minimum": 1, "description": "CipherSaber-2 key-setup iterations: the number of times RC4's KSA mixing loop runs (the spec recommends 20). Must match on encrypt and decrypt; 1 = CipherSaber-1." },
                    "iv": { "type": "string", "description": "Optional explicit 10-byte IV, encoded per `format` (hex/base64). Leave empty to generate a fresh random IV (recommended). Used for deterministic/interop output. Ignored on decrypt (the IV is read from the ciphertext)." },
                    "format": { "type": "string", "enum": ["hex", "base64"], "default": "hex", "description": "Encoding for the ciphertext (and the key/iv when encoded): hex (default) or base64. The plaintext is always UTF-8 text." }
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
