//! gizza-ai/sops-encrypt — chat skill block on the shared tool abstraction.
//!
//! Structure-preserving encryption for YAML, JSON and `.env` documents: only the
//! leaf **values** are replaced with `ENC[GZAE1,...]` markers, so every key stays
//! readable and the file stays valid and diffable. The chat schema is
//! single-sourced from `descriptor()` (which also drives the CLI); `handle()`
//! delegates to `block_utils::run_skill`.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

use gizza_ai_sops_encrypt_core as core;

#[derive(Deserialize)]
struct Args {
    document: String,
    passphrase: String,
    #[serde(default = "default_mode")]
    mode: String,
    #[serde(default = "default_format")]
    format: String,
    #[serde(default)]
    encrypted_suffix: String,
    #[serde(default = "default_unencrypted_suffix")]
    unencrypted_suffix: String,
    #[serde(default)]
    encrypted_regex: String,
    #[serde(default)]
    unencrypted_regex: String,
}

fn default_mode() -> String {
    "encrypt".to_string()
}

fn default_format() -> String {
    "auto".to_string()
}

fn default_unencrypted_suffix() -> String {
    core::DEFAULT_UNENCRYPTED_SUFFIX.to_string()
}

#[derive(Debug, Serialize)]
struct Resp {
    /// The rewritten document.
    result: String,
    mode: String,
    /// The format actually used: "yaml", "json" or "env".
    format: String,
    /// How many leaf values were encrypted or decrypted.
    values: usize,
}

impl Args {
    fn run(&self) -> Result<Resp, String> {
        let mode = match self.mode.trim().to_ascii_lowercase().as_str() {
            "" | "encrypt" => "encrypt",
            "decrypt" => "decrypt",
            other => {
                return Err(format!(
                    "unknown mode '{other}' (use 'encrypt' or 'decrypt')"
                ))
            }
        };
        let opts = core::Options {
            format: Some(core::Format::parse(&self.format)?),
            encrypted_suffix: self.encrypted_suffix.trim().to_string(),
            unencrypted_suffix: self.unencrypted_suffix.trim().to_string(),
            encrypted_regex: self.encrypted_regex.trim().to_string(),
            unencrypted_regex: self.unencrypted_regex.trim().to_string(),
        };
        let outcome = if mode == "encrypt" {
            core::encrypt(&self.document, &self.passphrase, &opts)?
        } else {
            core::decrypt(&self.document, &self.passphrase, &opts)?
        };
        Ok(Resp {
            result: outcome.document,
            mode: mode.to_string(),
            format: outcome.format.to_string(),
            values: outcome.values,
        })
    }
}

/// Single source for the chat schema (and CLI). `document` and `passphrase` are
/// required; every selection option falls back to the documented default, so a
/// bare paste encrypts every value except `*_unencrypted` keys.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("document").required().describe(
            "The YAML, JSON or .env document to rewrite. Paste the file contents; the format is \
             auto-detected unless format is set. Keys stay readable — only leaf values are \
             replaced with ENC[GZAE1,...] markers, and a metadata block carrying the KDF salt is \
             appended so the same tool can decrypt it later.",
        ))
        .param(Param::string("passphrase").required().describe(
            "Passphrase the per-document AES-256-GCM key is derived from (PBKDF2-HMAC-SHA256, \
             200000 iterations). The same passphrase is needed to decrypt. Treat it as a secret: \
             do not put it in a shared link, a bookmark, a ticket, or anything else that gets \
             pasted alongside the encrypted document.",
        ))
        .param(
            Param::enumv("mode", ["encrypt", "decrypt"])
                .default("encrypt")
                .describe(
                    "encrypt replaces the selected leaf values with ENC[GZAE1,...] markers; \
                     decrypt restores a document this tool encrypted, including the original \
                     scalar types.",
                ),
        )
        .param(
            Param::enumv("format", ["auto", "yaml", "json", "env"])
                .default("auto")
                .describe(
                    "Document format. auto (default) detects JSON by its opening brace, .env when \
                     every meaningful line is a KEY=VALUE assignment, and YAML otherwise.",
                ),
        )
        .param(Param::string("encrypted_suffix").describe(
            "Encrypt ONLY values whose key, or an ancestor key, ends with this suffix (e.g. \
             _secret). Leave empty to encrypt everything the unencrypted rules do not exempt. \
             Only one of encrypted_suffix, unencrypted_suffix, encrypted_regex and \
             unencrypted_regex may be set at a time. Ignored when mode=decrypt.",
        ))
        .param(
            Param::string("unencrypted_suffix")
                .default(core::DEFAULT_UNENCRYPTED_SUFFIX)
                .describe(
                    "Leave values whose key, or an ancestor key, ends with this suffix in the \
                     clear. Defaults to _unencrypted; set it to an empty string to encrypt every \
                     value. Ignored when mode=decrypt.",
                ),
        )
        .param(Param::string("encrypted_regex").describe(
            "Encrypt ONLY values whose key, or an ancestor key, matches this regular expression \
             (e.g. ^(password|token|.*_key)$). Ignored when mode=decrypt.",
        ))
        .param(Param::string("unencrypted_regex").describe(
            "Leave values whose key, or an ancestor key, matches this regular expression in the \
             clear (e.g. ^(host|region|port)$). Ignored when mode=decrypt.",
        ))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/sops-encrypt",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Encrypt or decrypt config values, keeping keys readable",
    skill(
        description = "Encrypt or decrypt the values of a YAML, JSON or .env document with a passphrase, leaving every key readable. Each selected leaf value is replaced with an ENC[GZAE1,data:...,iv:...,tag:...,type:...] marker (AES-256-GCM, one PBKDF2-HMAC-SHA256 key per document, the value's key path as authenticated data), so the file stays valid, stays diffable, and a ciphertext cannot be moved to another key. Pick which values are covered with encrypted_suffix, unencrypted_suffix (default _unencrypted), encrypted_regex or unencrypted_regex — one at a time. mode=decrypt reverses a document this tool produced and restores the original scalar types. This is a passphrase-based format, not the sops binary's KMS/age/PGP format, so the output is not interchangeable with the sops CLI.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "sops-encrypt", |a: Args| {
            a.run().map_err(SkillError::InvalidArgs)
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
                    "document": { "type": "string", "description": "The YAML, JSON or .env document to rewrite. Paste the file contents; the format is auto-detected unless format is set. Keys stay readable — only leaf values are replaced with ENC[GZAE1,...] markers, and a metadata block carrying the KDF salt is appended so the same tool can decrypt it later." },
                    "passphrase": { "type": "string", "description": "Passphrase the per-document AES-256-GCM key is derived from (PBKDF2-HMAC-SHA256, 200000 iterations). The same passphrase is needed to decrypt. Treat it as a secret: do not put it in a shared link, a bookmark, a ticket, or anything else that gets pasted alongside the encrypted document." },
                    "mode": { "type": "string", "enum": ["encrypt", "decrypt"], "default": "encrypt", "description": "encrypt replaces the selected leaf values with ENC[GZAE1,...] markers; decrypt restores a document this tool encrypted, including the original scalar types." },
                    "format": { "type": "string", "enum": ["auto", "yaml", "json", "env"], "default": "auto", "description": "Document format. auto (default) detects JSON by its opening brace, .env when every meaningful line is a KEY=VALUE assignment, and YAML otherwise." },
                    "encrypted_suffix": { "type": "string", "description": "Encrypt ONLY values whose key, or an ancestor key, ends with this suffix (e.g. _secret). Leave empty to encrypt everything the unencrypted rules do not exempt. Only one of encrypted_suffix, unencrypted_suffix, encrypted_regex and unencrypted_regex may be set at a time. Ignored when mode=decrypt." },
                    "unencrypted_suffix": { "type": "string", "default": "_unencrypted", "description": "Leave values whose key, or an ancestor key, ends with this suffix in the clear. Defaults to _unencrypted; set it to an empty string to encrypt every value. Ignored when mode=decrypt." },
                    "encrypted_regex": { "type": "string", "description": "Encrypt ONLY values whose key, or an ancestor key, matches this regular expression (e.g. ^(password|token|.*_key)$). Ignored when mode=decrypt." },
                    "unencrypted_regex": { "type": "string", "description": "Leave values whose key, or an ancestor key, matches this regular expression in the clear (e.g. ^(host|region|port)$). Ignored when mode=decrypt." }
                },
                "required": ["document", "passphrase"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    fn args(document: &str, mode: &str) -> Args {
        Args {
            document: document.to_string(),
            passphrase: "hunter2".to_string(),
            mode: mode.to_string(),
            format: default_format(),
            encrypted_suffix: String::new(),
            unencrypted_suffix: default_unencrypted_suffix(),
            encrypted_regex: String::new(),
            unencrypted_regex: String::new(),
        }
    }

    #[test]
    fn encrypt_then_decrypt_round_trips_through_the_args_layer() {
        let src = "app: demo\ndatabase:\n  password: s3cr3t\n  port: 5432\n";
        let enc = args(src, "encrypt").run().unwrap();
        assert_eq!(enc.mode, "encrypt");
        assert_eq!(enc.format, "yaml");
        assert_eq!(enc.values, 3);
        assert!(enc.result.contains("password: ENC[GZAE1,"));
        assert!(!enc.result.contains("s3cr3t"));

        let dec = args(&enc.result, "decrypt").run().unwrap();
        assert_eq!(dec.mode, "decrypt");
        assert_eq!(dec.values, 3);
        assert!(dec.result.contains("password: s3cr3t"));
        assert!(dec.result.contains("port: 5432"));
        assert!(!dec.result.contains("gizza_sops"));
    }

    #[test]
    fn unknown_mode_is_rejected() {
        let err = args("a: b\n", "sign").run().unwrap_err();
        assert!(err.contains("unknown mode 'sign'"), "got: {err}");
    }

    #[test]
    fn unknown_format_is_rejected() {
        let mut a = args("a: b\n", "encrypt");
        a.format = "toml".into();
        let err = a.run().unwrap_err();
        assert!(err.contains("unknown format 'toml'"), "got: {err}");
    }

    #[test]
    fn an_empty_unencrypted_suffix_encrypts_every_value() {
        let mut a = args("region_unencrypted: eu-west-1\npassword: p\n", "encrypt");
        a.unencrypted_suffix = String::new();
        let enc = a.run().unwrap();
        assert_eq!(enc.values, 2);
        assert!(!enc.result.contains("eu-west-1"));
    }
}
