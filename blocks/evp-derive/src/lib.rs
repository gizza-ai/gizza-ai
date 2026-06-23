//! gizza-ai/evp-derive — reproduce OpenSSL's legacy `EVP_BytesToKey` key/IV
//! derivation from a password, salt, and hash. Thin wrapper; chat schema
//! single-sourced from descriptor(); handler delegates to run_skill. Pure → all
//! backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_evp_derive_core::derive;
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    password: String,
    #[serde(default)]
    salt: String,
    #[serde(default = "default_salt_encoding")]
    salt_encoding: String,
    #[serde(default = "default_hash")]
    hash: String,
    #[serde(default = "default_key_length")]
    key_length: u32,
    #[serde(default = "default_iv_length")]
    iv_length: u32,
    #[serde(default = "default_count")]
    count: u32,
    #[serde(default = "default_encoding")]
    encoding: String,
}
fn default_salt_encoding() -> String {
    "utf8".to_string()
}
fn default_hash() -> String {
    "md5".to_string()
}
fn default_key_length() -> u32 {
    32
}
fn default_iv_length() -> u32 {
    16
}
fn default_count() -> u32 {
    1
}
fn default_encoding() -> String {
    "hex".to_string()
}

#[derive(Serialize)]
struct Resp {
    key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    iv: Option<String>,
    hash: String,
    key_length: u32,
    iv_length: u32,
    count: u32,
    encoding: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("password").required().describe("The password (passphrase) to derive the key and IV from."))
        .param(Param::string("salt").describe("The salt. Defaults to empty (OpenSSL's -nosalt). With OpenSSL's -S flag the salt is 8 bytes; give those bytes here (e.g. as hex)."))
        .param(
            Param::enumv("salt_encoding", ["utf8", "hex", "base64"])
                .default("utf8")
                .describe("How to interpret the salt string: utf8 text (default), hex, or base64."),
        )
        .param(
            Param::enumv("hash", ["md5", "sha1", "sha256", "sha512"])
                .default("md5")
                .describe("Message digest used by EVP_BytesToKey (OpenSSL's -md). md5 is the historical default; modern OpenSSL defaults to sha256."),
        )
        .param(
            Param::integer("key_length")
                .min(0.0)
                .max(1024.0)
                .describe("Derived key length in bytes (default 32 = AES-256). 16 = AES-128, 24 = AES-192, 8 = DES."),
        )
        .param(
            Param::integer("iv_length")
                .min(0.0)
                .max(1024.0)
                .describe("Derived IV length in bytes (default 16 = AES block size). Use 0 for ciphers/modes that need no IV."),
        )
        .param(
            Param::integer("count")
                .min(1.0)
                .max(10000000.0)
                .describe("Iteration count (OpenSSL's -iter, default 1). Each digest block is hashed count times."),
        )
        .param(
            Param::enumv("encoding", ["hex", "base64"])
                .default("hex")
                .describe("Output encoding of the key and IV: hex (default) or base64."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct EvpDerive;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/evp-derive",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Reproduce OpenSSL's legacy EVP_BytesToKey key/IV derivation",
    skill(
        description = "Reproduce OpenSSL's legacy EVP_BytesToKey key/IV derivation from a password, salt, and hash — the KDF behind `openssl enc -pass pass:… [-S salt]` without -pbkdf2. Returns the derived key (and IV) as hex (default) or base64. Choose the digest (md5 default, sha1, sha256, sha512), the salt (with salt_encoding utf8/hex/base64), the key length in bytes (default 32 = AES-256), the IV length in bytes (default 16; use 0 for none), and the iteration count (default 1). Deterministic — the same inputs always produce the same key and IV. Use it to recover the exact key/IV OpenSSL used to encrypt a file so you can decrypt it elsewhere. Runs locally; the password never leaves the device.",
        parameters = schema_json()
    ),
)]
impl EvpDerive {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "evp-derive", |a: Args| {
            let (key, iv) = derive(
                &a.password,
                &a.salt,
                &a.salt_encoding,
                &a.hash,
                a.key_length as usize,
                a.iv_length as usize,
                a.count,
                &a.encoding,
            )
            .map_err(SkillError::InvalidArgs)?;
            Ok(Resp {
                key,
                iv: if iv.is_empty() { None } else { Some(iv) },
                hash: a.hash.trim().to_ascii_lowercase(),
                key_length: a.key_length,
                iv_length: a.iv_length,
                count: a.count,
                encoding: a.encoding.trim().to_ascii_lowercase(),
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
                    "password": { "type": "string", "description": "The password (passphrase) to derive the key and IV from." },
                    "salt": { "type": "string", "description": "The salt. Defaults to empty (OpenSSL's -nosalt). With OpenSSL's -S flag the salt is 8 bytes; give those bytes here (e.g. as hex)." },
                    "salt_encoding": { "type": "string", "enum": ["utf8", "hex", "base64"], "default": "utf8", "description": "How to interpret the salt string: utf8 text (default), hex, or base64." },
                    "hash": { "type": "string", "enum": ["md5", "sha1", "sha256", "sha512"], "default": "md5", "description": "Message digest used by EVP_BytesToKey (OpenSSL's -md). md5 is the historical default; modern OpenSSL defaults to sha256." },
                    "key_length": { "type": "integer", "minimum": 0, "maximum": 1024, "description": "Derived key length in bytes (default 32 = AES-256). 16 = AES-128, 24 = AES-192, 8 = DES." },
                    "iv_length": { "type": "integer", "minimum": 0, "maximum": 1024, "description": "Derived IV length in bytes (default 16 = AES block size). Use 0 for ciphers/modes that need no IV." },
                    "count": { "type": "integer", "minimum": 1, "maximum": 10000000, "description": "Iteration count (OpenSSL's -iter, default 1). Each digest block is hashed count times." },
                    "encoding": { "type": "string", "enum": ["hex", "base64"], "default": "hex", "description": "Output encoding of the key and IV: hex (default) or base64." }
                },
                "required": ["password"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
