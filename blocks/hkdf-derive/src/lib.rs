//! gizza-ai/hkdf-derive — HKDF extract-and-expand key derivation (RFC 5869) with
//! a selectable HMAC hash, input key material (IKM), optional salt, optional info
//! (context label), and output length. Thin wrapper; chat schema single-sourced
//! from descriptor(); handler delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_hkdf_derive_core::{derive, extract};
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    ikm: String,
    #[serde(default = "default_mode")]
    mode: String,
    #[serde(default = "default_text_encoding")]
    ikm_encoding: String,
    #[serde(default)]
    salt: String,
    #[serde(default = "default_text_encoding")]
    salt_encoding: String,
    #[serde(default)]
    info: String,
    #[serde(default = "default_text_encoding")]
    info_encoding: String,
    #[serde(default = "default_hash")]
    hash: String,
    #[serde(default = "default_length")]
    length: u32,
    #[serde(default = "default_encoding")]
    encoding: String,
}
fn default_mode() -> String {
    "derive".to_string()
}
fn default_text_encoding() -> String {
    "utf8".to_string()
}
fn default_hash() -> String {
    "sha256".to_string()
}
fn default_length() -> u32 {
    32
}
fn default_encoding() -> String {
    "hex".to_string()
}

#[derive(Serialize)]
struct Resp {
    #[serde(skip_serializing_if = "Option::is_none")]
    key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    prk: Option<String>,
    mode: String,
    hash: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    length: Option<u32>,
    encoding: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("ikm").required().describe("Input key material (IKM): the secret/source key to derive from. Interpreted per `ikm_encoding`."))
        .param(
            Param::enumv("mode", ["derive", "extract"])
                .default("derive")
                .describe("derive (default) runs full HKDF extract-then-expand to produce `length` bytes; extract returns only the intermediate pseudorandom key (PRK)."),
        )
        .param(
            Param::enumv("ikm_encoding", ["utf8", "hex", "base64"])
                .default("utf8")
                .describe("How to interpret the IKM string: utf8 text (default), hex, or base64."),
        )
        .param(Param::string("salt").describe("Optional salt. If empty, HKDF uses HashLen zero bytes (per RFC 5869). A non-secret random salt is recommended."))
        .param(
            Param::enumv("salt_encoding", ["utf8", "hex", "base64"])
                .default("utf8")
                .describe("How to interpret the salt string: utf8 text (default), hex, or base64."),
        )
        .param(Param::string("info").describe("Optional context/application info string that binds the output to a use (e.g. \"app:tls key\"). Empty by default."))
        .param(
            Param::enumv("info_encoding", ["utf8", "hex", "base64"])
                .default("utf8")
                .describe("How to interpret the info string: utf8 text (default), hex, or base64."),
        )
        .param(
            Param::enumv("hash", ["sha1", "sha256", "sha384", "sha512"])
                .default("sha256")
                .describe("HMAC hash function (default sha256). sha1 is for legacy/interop only."),
        )
        .param(
            Param::integer("length")
                .min(1.0)
                .max(8160.0)
                .describe("Derived key length in bytes (derive mode, default 32). Max is 255 * HashLen for the chosen hash."),
        )
        .param(
            Param::enumv("encoding", ["hex", "base64"])
                .default("hex")
                .describe("Output encoding of the derived key / PRK: hex (default) or base64."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct HkdfDerive;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/hkdf-derive",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Derive a key with HKDF (extract-and-expand, RFC 5869)",
    skill(
        description = "Derive a cryptographic key from input key material (IKM) using HKDF, the HMAC-based extract-and-expand key derivation function (RFC 5869). mode=derive (default) runs the full extract-then-expand and outputs `length` bytes; mode=extract returns only the intermediate pseudorandom key (PRK). Choose the hash (sha1, sha256 default, sha384, sha512), an optional salt, an optional info/context string (each with its own utf8/hex/base64 encoding), the output length in bytes (default 32, max 255*HashLen), and the output encoding (hex default or base64). HKDF is the standard for splitting one strong secret into multiple keys (used in TLS 1.3, Signal, Noise); it is NOT a password hash — use PBKDF2/scrypt/Argon2 for low-entropy passwords. Deterministic and runs locally; the secret never leaves the device.",
        parameters = schema_json()
    ),
)]
impl HkdfDerive {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "hkdf-derive", |a: Args| {
            let hash_label = a.hash.trim().to_ascii_lowercase();
            let enc_label = a.encoding.trim().to_ascii_lowercase();
            match a.mode.trim().to_ascii_lowercase().as_str() {
                "derive" | "" => {
                    let key = derive(
                        &a.ikm,
                        &a.ikm_encoding,
                        &a.salt,
                        &a.salt_encoding,
                        &a.info,
                        &a.info_encoding,
                        &a.hash,
                        a.length as usize,
                        &a.encoding,
                    )
                    .map_err(SkillError::InvalidArgs)?;
                    Ok(Resp {
                        key: Some(key),
                        prk: None,
                        mode: "derive".to_string(),
                        hash: hash_label,
                        length: Some(a.length),
                        encoding: enc_label,
                    })
                }
                "extract" => {
                    let prk = extract(
                        &a.ikm,
                        &a.ikm_encoding,
                        &a.salt,
                        &a.salt_encoding,
                        &a.hash,
                        &a.encoding,
                    )
                    .map_err(SkillError::InvalidArgs)?;
                    Ok(Resp {
                        key: None,
                        prk: Some(prk),
                        mode: "extract".to_string(),
                        hash: hash_label,
                        length: None,
                        encoding: enc_label,
                    })
                }
                other => Err(SkillError::InvalidArgs(format!(
                    "unknown mode '{other}' (use derive or extract)"
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
                    "ikm": { "type": "string", "description": "Input key material (IKM): the secret/source key to derive from. Interpreted per `ikm_encoding`." },
                    "mode": { "type": "string", "enum": ["derive", "extract"], "default": "derive", "description": "derive (default) runs full HKDF extract-then-expand to produce `length` bytes; extract returns only the intermediate pseudorandom key (PRK)." },
                    "ikm_encoding": { "type": "string", "enum": ["utf8", "hex", "base64"], "default": "utf8", "description": "How to interpret the IKM string: utf8 text (default), hex, or base64." },
                    "salt": { "type": "string", "description": "Optional salt. If empty, HKDF uses HashLen zero bytes (per RFC 5869). A non-secret random salt is recommended." },
                    "salt_encoding": { "type": "string", "enum": ["utf8", "hex", "base64"], "default": "utf8", "description": "How to interpret the salt string: utf8 text (default), hex, or base64." },
                    "info": { "type": "string", "description": "Optional context/application info string that binds the output to a use (e.g. \"app:tls key\"). Empty by default." },
                    "info_encoding": { "type": "string", "enum": ["utf8", "hex", "base64"], "default": "utf8", "description": "How to interpret the info string: utf8 text (default), hex, or base64." },
                    "hash": { "type": "string", "enum": ["sha1", "sha256", "sha384", "sha512"], "default": "sha256", "description": "HMAC hash function (default sha256). sha1 is for legacy/interop only." },
                    "length": { "type": "integer", "minimum": 1, "maximum": 8160, "description": "Derived key length in bytes (derive mode, default 32). Max is 255 * HashLen for the chosen hash." },
                    "encoding": { "type": "string", "enum": ["hex", "base64"], "default": "hex", "description": "Output encoding of the derived key / PRK: hex (default) or base64." }
                },
                "required": ["ikm"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
