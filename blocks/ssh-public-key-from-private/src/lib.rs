//! gizza-ai/ssh-public-key-from-private — chat skill block on the shared tool abstraction.
//! Derives the OpenSSH public-key line (id_*.pub format) from a private key. The
//! chat schema is single-sourced from descriptor() (which also drives the CLI);
//! handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_ssh_public_key_from_private_core::{parse_der_format, parse_key_type, run_with};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default)]
    key_type: String,
    #[serde(default)]
    der_format: String,
    #[serde(default)]
    comment: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("The PRIVATE key to derive the public key from: a PEM block (-----BEGIN PRIVATE KEY-----, RSA PRIVATE KEY, or EC PRIVATE KEY), or the raw DER bytes as hex/base64. OpenSSH-format private keys are not supported (convert with: ssh-keygen -p -m PEM -f key)."),
        )
        .param(
            Param::enumv("key_type", ["auto", "rsa", "ec", "ed25519"])
                .default("auto")
                .describe("Key algorithm: auto (default) detects it from the PEM label and otherwise tries each; rsa, ec (NIST P-256/P-384), or ed25519. Required only to disambiguate raw DER input."),
        )
        .param(
            Param::enumv("der_format", ["hex", "base64"])
                .default("hex")
                .describe("How to interpret raw (non-PEM) DER input bytes: hex (default) or base64. Ignored when the input is PEM."),
        )
        .param(
            Param::string("comment")
                .describe("Optional comment appended to the key line (e.g. user@host), as the trailing field of an OpenSSH .pub line. Default: none."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/ssh-public-key-from-private",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Derive the OpenSSH public key (.pub line) from a private key",
    skill(
        description = "Derive the OpenSSH PUBLIC key — the single-line id_*.pub / authorized_keys format (e.g. 'ssh-rsa AAAA... user@host') — from a PRIVATE key. This is the offline equivalent of 'ssh-keygen -y -f key'. Supports RSA (ssh-rsa), EC NIST P-256/P-384 (ecdsa-sha2-nistp256/384) and Ed25519 (ssh-ed25519) private keys, supplied as PEM text (PKCS#8 'PRIVATE KEY', PKCS#1 'RSA PRIVATE KEY', or SEC1 'EC PRIVATE KEY') or as raw DER bytes given via der_format as hex or base64. key_type defaults to auto (detect from the PEM label, otherwise try each). An optional comment is appended as the trailing field. OpenSSH-format private keys (-----BEGIN OPENSSH PRIVATE KEY-----) are NOT supported — convert them to PEM first. The private key never leaves the machine.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "ssh-public-key-from-private", |a: Args| {
            let kt = parse_key_type(&a.key_type).map_err(SkillError::InvalidArgs)?;
            let fmt = parse_der_format(&a.der_format).map_err(SkillError::InvalidArgs)?;
            run_with(&a.input, kt, fmt, &a.comment).map_err(SkillError::InvalidArgs)
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
                    "input":      { "type": "string", "description": "The PRIVATE key to derive the public key from: a PEM block (-----BEGIN PRIVATE KEY-----, RSA PRIVATE KEY, or EC PRIVATE KEY), or the raw DER bytes as hex/base64. OpenSSH-format private keys are not supported (convert with: ssh-keygen -p -m PEM -f key)." },
                    "key_type":   { "type": "string", "enum": ["auto", "rsa", "ec", "ed25519"], "default": "auto", "description": "Key algorithm: auto (default) detects it from the PEM label and otherwise tries each; rsa, ec (NIST P-256/P-384), or ed25519. Required only to disambiguate raw DER input." },
                    "der_format": { "type": "string", "enum": ["hex", "base64"], "default": "hex", "description": "How to interpret raw (non-PEM) DER input bytes: hex (default) or base64. Ignored when the input is PEM." },
                    "comment":    { "type": "string", "description": "Optional comment appended to the key line (e.g. user@host), as the trailing field of an OpenSSH .pub line. Default: none." }
                },
                "required": ["input"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
