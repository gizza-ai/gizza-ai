//! gizza-ai/chacha20-cipher — encrypt or decrypt data with the ChaCha20 stream
//! cipher and the ChaCha20-Poly1305 AEAD construction (RFC 8439). Thin wrapper;
//! chat schema single-sourced from descriptor(); handler delegates to run_skill.
//! Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_chacha20_cipher_core::{decrypt, encrypt, Encoding, KeyFormat, Mode};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default = "default_op")]
    operation: String,
    key: String,
    nonce: String,
    #[serde(default)]
    aad: String,
    #[serde(default = "default_mode")]
    mode: String,
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
fn default_mode() -> String {
    "stream".to_string()
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
    mode: String,
    counter: u64,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe(
            "For encrypt: the plaintext (UTF-8). For decrypt: the ciphertext, encoded with `format` (in aead mode the encoded value is ciphertext followed by the 16-byte Poly1305 tag).",
        ))
        .param(
            Param::enumv("operation", ["encrypt", "decrypt"]).default("encrypt").describe(
                "encrypt (default) or decrypt.",
            ),
        )
        .param(Param::string("key").required().describe(
            "The 32-byte (256-bit) ChaCha20 key: a 32-char UTF-8 passphrase when key_format=text (default), or a 64-char hex (or base64) string when key_format=encoded.",
        ))
        .param(Param::string("nonce").required().describe(
            "The 12-byte (96-bit) nonce/IV (IETF/RFC 8439). A 12-char UTF-8 string when key_format=text, or a 24-char hex (or base64) value when key_format=encoded. Must match on encrypt and decrypt, and must be unique per message for a given key.",
        ))
        .param(Param::string("aad").describe(
            "Optional associated data (UTF-8) for aead mode — authenticated but not encrypted. Must match on encrypt and decrypt. Ignored in stream mode.",
        ))
        .param(
            Param::enumv("mode", ["stream", "aead"]).default("stream").describe(
                "stream = raw ChaCha20 (unauthenticated keystream XOR; default); aead = ChaCha20-Poly1305 authenticated encryption with a 16-byte tag and optional AAD.",
            ),
        )
        .param(
            Param::enumv("key_format", ["text", "encoded"]).default("text").describe(
                "How to read the key and nonce: text = UTF-8 (default); encoded = hex/base64 per `format`.",
            ),
        )
        .param(
            Param::integer("counter").default(0).min(0.0).describe(
                "Initial 32-bit block counter for stream mode (each block is 64 bytes). Default 0. Ignored in aead mode (RFC 8439 fixes the counter). Must match on encrypt and decrypt.",
            ),
        )
        .param(
            Param::enumv("format", ["hex", "base64"]).default("hex").describe(
                "Encoding for the ciphertext/tag (and the key/nonce when key_format=encoded): hex (default) or base64. The plaintext is always UTF-8 text.",
            ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ChaCha20Cipher;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/chacha20-cipher",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Encrypt or decrypt data with ChaCha20 or ChaCha20-Poly1305 (RFC 8439)",
    skill(
        description = "Encrypt or decrypt data with the ChaCha20 stream cipher or the ChaCha20-Poly1305 AEAD construction (RFC 8439, IETF variant: 256-bit key, 96-bit/12-byte nonce, 32-bit counter). operation = encrypt (default) or decrypt. mode = stream (raw ChaCha20 keystream XOR, unauthenticated; default) or aead (ChaCha20-Poly1305 — authenticated encryption that appends a 16-byte Poly1305 tag and authenticates optional `aad`; decryption verifies the tag and fails if anything was tampered with). key must be 32 bytes and nonce exactly 12 bytes, read as UTF-8 (key_format=text, default) or decoded from hex/base64 (key_format=encoded). counter is the initial 32-bit block counter for stream mode (default 0; ignored for aead). The ciphertext/tag (and an encoded key/nonce) use format = hex (default) or base64; the plaintext is UTF-8. The same key+nonce must be used to decrypt, and a key+nonce pair must never be reused for two different messages. For password-based file encryption use aes-cipher or text-encrypt instead.",
        parameters = schema_json()
    ),
)]
impl ChaCha20Cipher {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "chacha20-cipher", |a: Args| {
            let key_format = KeyFormat::parse(&a.key_format).map_err(SkillError::InvalidArgs)?;
            let mode = Mode::parse(&a.mode).map_err(SkillError::InvalidArgs)?;
            let fmt = Encoding::parse(&a.format).map_err(SkillError::InvalidArgs)?;
            if a.counter < 0.0 {
                return Err(SkillError::InvalidArgs("counter must be >= 0".into()));
            }
            let counter = a.counter as u32;
            let output = match a.operation.trim().to_ascii_lowercase().as_str() {
                "encrypt" | "" => {
                    encrypt(&a.data, &a.key, &a.nonce, &a.aad, key_format, mode, counter, fmt)
                }
                "decrypt" => {
                    decrypt(&a.data, &a.key, &a.nonce, &a.aad, key_format, mode, counter, fmt)
                }
                other => Err(format!("unknown operation '{other}' (use encrypt or decrypt)")),
            }
            .map_err(SkillError::InvalidArgs)?;
            Ok(Resp {
                output,
                operation: a.operation,
                mode: a.mode,
                counter: counter as u64,
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
                    "data": { "type": "string", "description": "For encrypt: the plaintext (UTF-8). For decrypt: the ciphertext, encoded with `format` (in aead mode the encoded value is ciphertext followed by the 16-byte Poly1305 tag)." },
                    "operation": { "type": "string", "enum": ["encrypt", "decrypt"], "default": "encrypt", "description": "encrypt (default) or decrypt." },
                    "key": { "type": "string", "description": "The 32-byte (256-bit) ChaCha20 key: a 32-char UTF-8 passphrase when key_format=text (default), or a 64-char hex (or base64) string when key_format=encoded." },
                    "nonce": { "type": "string", "description": "The 12-byte (96-bit) nonce/IV (IETF/RFC 8439). A 12-char UTF-8 string when key_format=text, or a 24-char hex (or base64) value when key_format=encoded. Must match on encrypt and decrypt, and must be unique per message for a given key." },
                    "aad": { "type": "string", "description": "Optional associated data (UTF-8) for aead mode — authenticated but not encrypted. Must match on encrypt and decrypt. Ignored in stream mode." },
                    "mode": { "type": "string", "enum": ["stream", "aead"], "default": "stream", "description": "stream = raw ChaCha20 (unauthenticated keystream XOR; default); aead = ChaCha20-Poly1305 authenticated encryption with a 16-byte tag and optional AAD." },
                    "key_format": { "type": "string", "enum": ["text", "encoded"], "default": "text", "description": "How to read the key and nonce: text = UTF-8 (default); encoded = hex/base64 per `format`." },
                    "counter": { "type": "integer", "default": 0, "minimum": 0, "description": "Initial 32-bit block counter for stream mode (each block is 64 bytes). Default 0. Ignored in aead mode (RFC 8439 fixes the counter). Must match on encrypt and decrypt." },
                    "format": { "type": "string", "enum": ["hex", "base64"], "default": "hex", "description": "Encoding for the ciphertext/tag (and the key/nonce when key_format=encoded): hex (default) or base64. The plaintext is always UTF-8 text." }
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
