//! gizza-ai/rncryptor-encrypt — chat skill block on the shared tool abstraction.
//!
//! Builds and opens the password-based **RNCryptor v3** container: the
//! self-describing blob (`0x03 0x01 | encryption salt | HMAC salt | IV |
//! ciphertext | HMAC-SHA256`) that the RNCryptor libraries shipping inside iOS,
//! Android and Python apps read and write. The chat schema is single-sourced
//! from `descriptor()` (which also drives the CLI); `handle()` delegates to
//! `block_utils::run_skill`.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

use gizza_ai_rncryptor_encrypt_core as core;

#[derive(Deserialize)]
struct Args {
    #[serde(default = "default_operation")]
    operation: String,
    data: String,
    password: String,
    #[serde(default = "default_data_encoding")]
    data_encoding: String,
    #[serde(default = "default_output_encoding")]
    output_encoding: String,
    #[serde(default)]
    encryption_salt: String,
    #[serde(default)]
    hmac_salt: String,
    #[serde(default)]
    iv: String,
}

fn default_operation() -> String {
    "encrypt".to_string()
}

fn default_data_encoding() -> String {
    "text".to_string()
}

fn default_output_encoding() -> String {
    "base64".to_string()
}

impl Args {
    fn run(&self) -> Result<String, String> {
        core::run(
            &self.operation,
            &self.data,
            &self.password,
            &self.data_encoding,
            &self.output_encoding,
            &self.encryption_salt,
            &self.hmac_salt,
            &self.iv,
        )
    }
}

/// Single source for the chat schema (and CLI). `data` and `password` are
/// required; everything else defaults, so a bare paste seals text with fresh
/// random salts and a fresh IV and returns base64.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::enumv("operation", ["encrypt", "decrypt"])
                .default("encrypt")
                .describe(
                    "encrypt (default) seals data into a new RNCryptor v3 password container; \
                     decrypt verifies an existing container's HMAC and returns the plaintext. \
                     Anything else is rejected.",
                ),
        )
        .param(Param::string("data").required().describe(
            "What to process: the plaintext to seal when operation=encrypt, or the container to \
             open when operation=decrypt. Read according to data_encoding. Capped at 4 MiB of \
             decoded bytes.",
        ))
        .param(Param::string("password").required().describe(
            "The passphrase both keys are derived from (PBKDF2-HMAC-SHA1, 10000 iterations, one \
             pass per salt). Any non-empty string, including non-ASCII; it is UTF-8 encoded \
             before derivation. The exact same password is needed to decrypt — there is no \
             recovery path.",
        ))
        .param(
            Param::enumv("data_encoding", ["text", "hex", "base64"])
                .default("text")
                .describe(
                    "How to read data. text (default) treats it as UTF-8 characters when \
                     encrypting and auto-detects hex vs base64 when decrypting; hex and base64 \
                     decode it to raw bytes first, which is how you seal binary input. \
                     Whitespace and a leading 0x are ignored in hex.",
                ),
        )
        .param(
            Param::enumv("output_encoding", ["base64", "hex"])
                .default("base64")
                .describe(
                    "How the result is printed. base64 (default) is the compact form to paste \
                     between systems; hex is byte-addressable for comparing against a spec test \
                     vector. On decrypt this applies only when the plaintext is not valid UTF-8 \
                     — readable text comes back as text.",
                ),
        )
        .param(Param::string("encryption_salt").describe(
            "Optional 8-byte encryption salt as 16 hex characters (e.g. 0001020304050607). Leave \
             empty for a fresh random salt, which is what you want for real data; set it only to \
             reproduce a known container byte for byte. Ignored when operation=decrypt (the salt \
             is read from the container).",
        ))
        .param(Param::string("hmac_salt").describe(
            "Optional 8-byte HMAC salt as 16 hex characters (e.g. 0102030405060708). Leave empty \
             for a fresh random salt. Must differ from encryption_salt in practice, since it \
             derives the separate authentication key. Ignored when operation=decrypt.",
        ))
        .param(Param::string("iv").describe(
            "Optional 16-byte AES-CBC initialization vector as 32 hex characters (e.g. \
             02030405060708090a0b0c0d0e0f0001). Leave empty for a fresh random IV — reusing an IV \
             with the same password and salts leaks whether two plaintexts share a prefix. \
             Ignored when operation=decrypt.",
        ))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/rncryptor-encrypt",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Build or open a password-based RNCryptor v3 container",
    skill(
        description = "Encrypt data into a password-based RNCryptor v3 container, or decrypt one back. The output is the complete self-describing blob RNCryptor libraries read: version 0x03, options 0x01, an 8-byte encryption salt, an 8-byte HMAC salt, a 16-byte IV, AES-256-CBC ciphertext with PKCS#7 padding, and a trailing HMAC-SHA256 over everything before it. Both keys come from the password via PBKDF2-HMAC-SHA1 at 10000 iterations; those parameters are fixed by the format and are not adjustable, because changing any of them produces a blob no RNCryptor library can open. Salts and the IV are random per run unless encryption_salt/hmac_salt/iv are supplied as hex, which makes a run reproducible against the spec's published test vectors. data_encoding reads the input as text, hex or base64 (hex/base64 for binary payloads; on decrypt, text auto-detects hex vs base64) and output_encoding prints the result as base64 or hex. operation=decrypt verifies the HMAC in constant time before unpadding, so a wrong password or a modified container fails loudly instead of returning garbage. Handles the password variant only, not key-based containers, and only version 3.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "rncryptor-encrypt", |a: Args| {
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
                    "operation": { "type": "string", "enum": ["encrypt", "decrypt"], "default": "encrypt", "description": "encrypt (default) seals data into a new RNCryptor v3 password container; decrypt verifies an existing container's HMAC and returns the plaintext. Anything else is rejected." },
                    "data": { "type": "string", "description": "What to process: the plaintext to seal when operation=encrypt, or the container to open when operation=decrypt. Read according to data_encoding. Capped at 4 MiB of decoded bytes." },
                    "password": { "type": "string", "description": "The passphrase both keys are derived from (PBKDF2-HMAC-SHA1, 10000 iterations, one pass per salt). Any non-empty string, including non-ASCII; it is UTF-8 encoded before derivation. The exact same password is needed to decrypt — there is no recovery path." },
                    "data_encoding": { "type": "string", "enum": ["text", "hex", "base64"], "default": "text", "description": "How to read data. text (default) treats it as UTF-8 characters when encrypting and auto-detects hex vs base64 when decrypting; hex and base64 decode it to raw bytes first, which is how you seal binary input. Whitespace and a leading 0x are ignored in hex." },
                    "output_encoding": { "type": "string", "enum": ["base64", "hex"], "default": "base64", "description": "How the result is printed. base64 (default) is the compact form to paste between systems; hex is byte-addressable for comparing against a spec test vector. On decrypt this applies only when the plaintext is not valid UTF-8 — readable text comes back as text." },
                    "encryption_salt": { "type": "string", "description": "Optional 8-byte encryption salt as 16 hex characters (e.g. 0001020304050607). Leave empty for a fresh random salt, which is what you want for real data; set it only to reproduce a known container byte for byte. Ignored when operation=decrypt (the salt is read from the container)." },
                    "hmac_salt": { "type": "string", "description": "Optional 8-byte HMAC salt as 16 hex characters (e.g. 0102030405060708). Leave empty for a fresh random salt. Must differ from encryption_salt in practice, since it derives the separate authentication key. Ignored when operation=decrypt." },
                    "iv": { "type": "string", "description": "Optional 16-byte AES-CBC initialization vector as 32 hex characters (e.g. 02030405060708090a0b0c0d0e0f0001). Leave empty for a fresh random IV — reusing an IV with the same password and salts leaks whether two plaintexts share a prefix. Ignored when operation=decrypt." }
                },
                "required": ["data", "password"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    fn args(operation: &str, data: &str) -> Args {
        Args {
            operation: operation.to_string(),
            data: data.to_string(),
            password: "thepassword".to_string(),
            data_encoding: default_data_encoding(),
            output_encoding: default_output_encoding(),
            encryption_salt: String::new(),
            hmac_salt: String::new(),
            iv: String::new(),
        }
    }

    #[test]
    fn encrypt_then_decrypt_round_trips_through_the_args_layer() {
        let sealed = args("encrypt", "attack at dawn").run().unwrap();
        assert_ne!(sealed, "attack at dawn");
        let opened = args("decrypt", &sealed).run().unwrap();
        assert_eq!(opened, "attack at dawn");
    }

    #[test]
    fn explicit_salts_and_iv_reproduce_the_spec_vector() {
        let mut a = args("encrypt", "01");
        a.data_encoding = "hex".into();
        a.output_encoding = "hex".into();
        a.encryption_salt = "0001020304050607".into();
        a.hmac_salt = "0102030405060708".into();
        a.iv = "02030405060708090a0b0c0d0e0f0001".into();
        assert_eq!(
            a.run().unwrap(),
            concat!(
                "0301",
                "0001020304050607",
                "0102030405060708",
                "02030405060708090a0b0c0d0e0f0001",
                "a1f8730e0bf480eb7b70f690abf21e02",
                "9514164ad3c474a51b30c7eaa1ca545b7de3de5b010acbad0a9a13857df696a8",
            )
        );
    }

    #[test]
    fn a_wrong_password_is_reported_not_silently_wrong() {
        let sealed = args("encrypt", "secret").run().unwrap();
        let mut a = args("decrypt", &sealed);
        a.password = "not the password".into();
        let err = a.run().unwrap_err();
        assert!(err.contains("HMAC verification failed"), "got: {err}");
    }

    #[test]
    fn unknown_operation_is_rejected() {
        let err = args("sign", "data").run().unwrap_err();
        assert!(err.contains("unknown operation"), "got: {err}");
    }
}
