//! gizza-ai/pem-public-key-extract — chat skill block on the shared tool abstraction.
//! Derives the SPKI public key from a private key. The chat schema is
//! single-sourced from descriptor() (which also drives the CLI); handle()
//! delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_pem_public_key_extract_core::{parse_der_format, parse_key_type, run};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default)]
    key_type: String,
    #[serde(default)]
    der_format: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("The PRIVATE key to derive the public key from: a PEM block (-----BEGIN PRIVATE KEY-----, RSA PRIVATE KEY, or EC PRIVATE KEY), or the raw DER bytes as hex/base64."),
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
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/pem-public-key-extract",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Extract the public key from a private key (PEM SPKI)",
    skill(
        description = "Compute the PUBLIC key from a PRIVATE key and output it as a PEM SubjectPublicKeyInfo block (-----BEGIN PUBLIC KEY-----). Supports RSA, EC (NIST P-256/P-384) and Ed25519 private keys, supplied as PEM text (PKCS#8 'PRIVATE KEY', PKCS#1 'RSA PRIVATE KEY', or SEC1 'EC PRIVATE KEY') or as raw DER bytes given via der_format as hex or base64. key_type defaults to auto (detect from the PEM label, otherwise try each algorithm) and only needs setting to disambiguate raw DER. This is the offline equivalent of 'openssl pkey -pubout'; the private key never leaves the machine.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "pem-public-key-extract", |a: Args| {
            let kt = parse_key_type(&a.key_type).map_err(SkillError::InvalidArgs)?;
            let fmt = parse_der_format(&a.der_format).map_err(SkillError::InvalidArgs)?;
            run(&a.input, kt, fmt).map_err(SkillError::InvalidArgs)
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
                    "input":      { "type": "string", "description": "The PRIVATE key to derive the public key from: a PEM block (-----BEGIN PRIVATE KEY-----, RSA PRIVATE KEY, or EC PRIVATE KEY), or the raw DER bytes as hex/base64." },
                    "key_type":   { "type": "string", "enum": ["auto", "rsa", "ec", "ed25519"], "default": "auto", "description": "Key algorithm: auto (default) detects it from the PEM label and otherwise tries each; rsa, ec (NIST P-256/P-384), or ed25519. Required only to disambiguate raw DER input." },
                    "der_format": { "type": "string", "enum": ["hex", "base64"], "default": "hex", "description": "How to interpret raw (non-PEM) DER input bytes: hex (default) or base64. Ignored when the input is PEM." }
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
