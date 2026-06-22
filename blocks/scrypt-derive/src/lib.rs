//! gizza-ai/scrypt-derive — derive or verify a key from a password using the
//! scrypt memory-hard KDF (RFC 7914) with configurable cost parameters N, r, p,
//! a salt, and output length. Thin wrapper; chat schema single-sourced from
//! descriptor(); handler delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_scrypt_derive_core::{derive, verify};
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
    #[serde(default = "default_n")]
    n: u32,
    #[serde(default = "default_r")]
    r: u32,
    #[serde(default = "default_p")]
    p: u32,
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
fn default_n() -> u32 {
    16384
}
fn default_r() -> u32 {
    8
}
fn default_p() -> u32 {
    1
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
    n: u32,
    r: u32,
    p: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    length: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    encoding: Option<String>,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("password")
                .required()
                .describe("The password (passphrase) to derive a key from, or to verify."),
        )
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
            Param::integer("n")
                .min(2.0)
                .max(1073741824.0)
                .describe("scrypt cost parameter N (CPU/memory cost). Must be a power of two greater than 1; default 16384. Memory used is about 128*N*r bytes."),
        )
        .param(
            Param::integer("r")
                .min(1.0)
                .max(1024.0)
                .describe("scrypt block size parameter r (default 8)."),
        )
        .param(
            Param::integer("p")
                .min(1.0)
                .max(1024.0)
                .describe("scrypt parallelization parameter p (default 1)."),
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
struct ScryptDerive;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/scrypt-derive",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Derive or verify a key from a password with scrypt",
    skill(
        description = "Derive a cryptographic key from a password using the scrypt memory-hard key-derivation function (RFC 7914), or verify a password reproduces a key. mode=derive (default) produces a key; mode=verify checks the password+params against the `expected` key. Configure the cost parameters N (power of two, default 16384), r (block size, default 8) and p (parallelization, default 1), the salt (with salt_encoding utf8/hex/base64), the output length in bytes (default 32), and the output encoding (hex default or base64). scrypt is memory-hard, resisting GPU/ASIC brute-force better than PBKDF2. Deterministic — the same inputs always produce the same key. Runs locally; the password never leaves the device.",
        parameters = schema_json()
    ),
)]
impl ScryptDerive {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "scrypt-derive", |a: Args| {
            match a.mode.trim().to_ascii_lowercase().as_str() {
                "derive" | "" => {
                    let key = derive(
                        &a.password,
                        &a.salt,
                        &a.salt_encoding,
                        a.n,
                        a.r,
                        a.p,
                        a.length as usize,
                        &a.encoding,
                    )
                    .map_err(SkillError::InvalidArgs)?;
                    Ok(Resp {
                        key: Some(key),
                        matches: None,
                        n: a.n,
                        r: a.r,
                        p: a.p,
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
                        a.n,
                        a.r,
                        a.p,
                        &a.expected,
                    )
                    .map_err(SkillError::InvalidArgs)?;
                    Ok(Resp {
                        key: None,
                        matches: Some(m),
                        n: a.n,
                        r: a.r,
                        p: a.p,
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
                    "n": { "type": "integer", "minimum": 2, "maximum": 1073741824, "description": "scrypt cost parameter N (CPU/memory cost). Must be a power of two greater than 1; default 16384. Memory used is about 128*N*r bytes." },
                    "r": { "type": "integer", "minimum": 1, "maximum": 1024, "description": "scrypt block size parameter r (default 8)." },
                    "p": { "type": "integer", "minimum": 1, "maximum": 1024, "description": "scrypt parallelization parameter p (default 1)." },
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
