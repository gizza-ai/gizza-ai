//! gizza-ai/ssh-keygen — generate an OpenSSH key pair (Ed25519 or RSA) in the
//! native `ssh-keygen(1)` wire formats: the `-----BEGIN OPENSSH PRIVATE KEY-----`
//! blob, the single-line authorized_keys public key, and the SHA256 fingerprint.
//!
//! Thin wrapper around the pure core; chat schema single-sourced from
//! descriptor(); handler delegates to run_skill. Pure Rust (getrandom CSPRNG) →
//! runs on all backends. Surfaces: chat + CLI. No standalone page — key
//! generation is non-deterministic, which doesn't fit the page's
//! live-recompute-on-input model (same as generate-rsa-key-pair /
//! ed25519-key-pair-generator).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    #[serde(default = "default_key_type")]
    key_type: String,
    #[serde(default = "default_bits")]
    bits: String,
    #[serde(default)]
    comment: String,
}
fn default_key_type() -> String {
    "ed25519".to_string()
}
fn default_bits() -> String {
    "2048".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::enumv("key_type", ["ed25519", "rsa"])
                .default("ed25519")
                .describe(
                    "Key algorithm: 'ed25519' (fast, modern, recommended) or 'rsa'. Default ed25519.",
                ),
        )
        .param(
            Param::enumv("bits", ["2048", "3072", "4096"])
                .default("2048")
                .describe(
                    "RSA modulus size in bits (2048, 3072, or 4096). Only used when key_type is 'rsa'; ignored for ed25519. Larger is more secure but slower. Default 2048.",
                ),
        )
        .param(
            Param::string("comment").describe(
                "Optional comment appended to the public-key line, e.g. 'user@host'. Leave blank for none.",
            ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct SshKeygen;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/ssh-keygen",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Generate an OpenSSH key pair (Ed25519 or RSA)",
    skill(
        description = "Generate a fresh OpenSSH key pair in the native ssh-keygen(1) formats: the private key as an -----BEGIN OPENSSH PRIVATE KEY----- blob, the public key as a single authorized_keys line (ssh-ed25519 AAAA... / ssh-rsa AAAA...), and the SHA256 fingerprint OpenSSH prints on load. Choose ed25519 (fast, modern, recommended; fixed 256-bit) or rsa with a 2048/3072/4096-bit modulus, and an optional comment (e.g. user@host). Keys are generated locally with a cryptographic RNG (getrandom); nothing is uploaded.",
        parameters = schema_json()
    ),
)]
impl SshKeygen {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "ssh-keygen", |a: Args| {
            let bits: usize = a.bits.trim().parse().map_err(|_| {
                SkillError::InvalidArgs(format!(
                    "invalid bits '{}'; expected 2048, 3072, or 4096",
                    a.bits
                ))
            })?;
            gizza_ai_ssh_keygen_core::generate(&a.key_type, bits, &a.comment)
                .map_err(SkillError::InvalidArgs)
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
                    "key_type": { "type": "string", "enum": ["ed25519", "rsa"], "default": "ed25519", "description": "Key algorithm: 'ed25519' (fast, modern, recommended) or 'rsa'. Default ed25519." },
                    "bits": { "type": "string", "enum": ["2048", "3072", "4096"], "default": "2048", "description": "RSA modulus size in bits (2048, 3072, or 4096). Only used when key_type is 'rsa'; ignored for ed25519. Larger is more secure but slower. Default 2048." },
                    "comment": { "type": "string", "description": "Optional comment appended to the public-key line, e.g. 'user@host'. Leave blank for none." }
                },
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
