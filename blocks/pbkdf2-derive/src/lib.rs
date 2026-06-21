//! gizza-ai/pbkdf2-derive — derive a key from a password using PBKDF2 (RFC 2898 /
//! RFC 8018) with a selectable HMAC hash, iteration count, salt, and output length.
//! Thin wrapper; chat schema single-sourced from descriptor(); handler delegates to
//! run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_pbkdf2_derive_core::{derive, verify};
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    password: String,
    #[serde(default = "default_mode")]
    mode: String,
    #[serde(default)]
    salt: String,
    #[serde(default = "default_salt_encoding")]
    salt_encoding: String,
    #[serde(default = "default_iterations")]
    iterations: u32,
    #[serde(default = "default_hash")]
    hash: String,
    #[serde(default = "default_dk_len")]
    length: u32,
    #[serde(default = "default_encoding")]
    encoding: String,
    #[serde(default)]
    expected: String,
}
fn default_mode() -> String {
    "derive".to_string()
}
fn default_salt_encoding() -> String {
    "utf8".to_string()
}
fn default_iterations() -> u32 {
    100000
}
fn default_hash() -> String {
    "sha256".to_string()
}
fn default_dk_len() -> u32 {
    32
}
fn default_encoding() -> String {
    "hex".to_string()
}

#[derive(Serialize)]
struct Resp {
    #[serde(skip_serializing_if = "Option::is_none")]
    key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "match")]
    matches: Option<bool>,
    hash: String,
    iterations: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    length: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    encoding: Option<String>,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("password").required().describe("The password (passphrase) to derive a key from, or to verify."))
        .param(
            Param::enumv("mode", ["derive", "verify"])
                .default("derive")
                .describe("derive (default) produces a key; verify checks the password+params against the `expected` key."),
        )
        .param(Param::string("salt").describe("The salt. Defaults to empty; for real key derivation use a unique random salt per key."))
        .param(
            Param::enumv("salt_encoding", ["utf8", "hex", "base64"])
                .default("utf8")
                .describe("How to interpret the salt string: utf8 text (default), hex, or base64."),
        )
        .param(
            Param::integer("iterations")
                .min(1.0)
                .max(10000000.0)
                .describe("Iteration count (default 100000). Higher is slower and more brute-force resistant; OWASP suggests 600000 for PBKDF2-HMAC-SHA256."),
        )
        .param(
            Param::enumv("hash", ["sha1", "sha256", "sha512"])
                .default("sha256")
                .describe("HMAC pseudorandom function (default sha256). sha1 is for legacy compatibility only."),
        )
        .param(
            Param::integer("length")
                .min(1.0)
                .max(1024.0)
                .describe("Derived key length in bytes (default 32)."),
        )
        .param(
            Param::enumv("encoding", ["hex", "base64"])
                .default("hex")
                .describe("Output encoding of the derived key (derive mode): hex (default) or base64."),
        )
        .param(
            Param::string("expected")
                .describe("The expected derived key (hex or base64, auto-detected) to check against (verify mode only). Its byte length sets the derived length."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Pbkdf2Derive;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/pbkdf2-derive",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Derive or verify a key from a password with PBKDF2",
    skill(
        description = "Derive a cryptographic key from a password using PBKDF2 (RFC 2898 / RFC 8018) with HMAC, or verify a password reproduces a key. mode=derive (default) produces a key; mode=verify checks the password+params against the `expected` key. Choose the hash (sha1, sha256 default, sha512), iteration count (default 100000), salt (with salt_encoding utf8/hex/base64), output length in bytes (default 32), and output encoding (hex default or base64). Deterministic — the same inputs always produce the same key. Runs locally; the password never leaves the device.",
        parameters = schema_json()
    ),
)]
impl Pbkdf2Derive {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "pbkdf2-derive", |a: Args| {
            let hash_label = a.hash.trim().to_ascii_lowercase();
            match a.mode.trim().to_ascii_lowercase().as_str() {
                "derive" | "" => {
                    let key = derive(
                        &a.password,
                        &a.salt,
                        &a.salt_encoding,
                        a.iterations,
                        &a.hash,
                        a.length as usize,
                        &a.encoding,
                    )
                    .map_err(SkillError::InvalidArgs)?;
                    Ok(Resp {
                        key: Some(key),
                        matches: None,
                        hash: hash_label,
                        iterations: a.iterations,
                        length: Some(a.length),
                        encoding: Some(a.encoding.trim().to_ascii_lowercase()),
                    })
                }
                "verify" => {
                    if a.expected.trim().is_empty() {
                        return Err(SkillError::InvalidArgs(
                            "verify mode requires the `expected` key".into(),
                        ));
                    }
                    let m = verify(
                        &a.password,
                        &a.salt,
                        &a.salt_encoding,
                        a.iterations,
                        &a.hash,
                        &a.expected,
                    )
                    .map_err(SkillError::InvalidArgs)?;
                    Ok(Resp {
                        key: None,
                        matches: Some(m),
                        hash: hash_label,
                        iterations: a.iterations,
                        length: None,
                        encoding: None,
                    })
                }
                other => Err(SkillError::InvalidArgs(format!(
                    "unknown mode '{other}' (use derive or verify)"
                ))),
            }
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
                    "password": { "type": "string", "description": "The password (passphrase) to derive a key from, or to verify." },
                    "mode": { "type": "string", "enum": ["derive", "verify"], "default": "derive", "description": "derive (default) produces a key; verify checks the password+params against the `expected` key." },
                    "salt": { "type": "string", "description": "The salt. Defaults to empty; for real key derivation use a unique random salt per key." },
                    "salt_encoding": { "type": "string", "enum": ["utf8", "hex", "base64"], "default": "utf8", "description": "How to interpret the salt string: utf8 text (default), hex, or base64." },
                    "iterations": { "type": "integer", "minimum": 1, "maximum": 10000000, "description": "Iteration count (default 100000). Higher is slower and more brute-force resistant; OWASP suggests 600000 for PBKDF2-HMAC-SHA256." },
                    "hash": { "type": "string", "enum": ["sha1", "sha256", "sha512"], "default": "sha256", "description": "HMAC pseudorandom function (default sha256). sha1 is for legacy compatibility only." },
                    "length": { "type": "integer", "minimum": 1, "maximum": 1024, "description": "Derived key length in bytes (default 32)." },
                    "encoding": { "type": "string", "enum": ["hex", "base64"], "default": "hex", "description": "Output encoding of the derived key (derive mode): hex (default) or base64." },
                    "expected": { "type": "string", "description": "The expected derived key (hex or base64, auto-detected) to check against (verify mode only). Its byte length sets the derived length." }
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
