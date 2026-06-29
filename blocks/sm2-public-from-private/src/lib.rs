//! gizza-ai/sm2-public-from-private — derive the SM2 public key from a private
//! key (Chinese national standard GM/T 0003, OSCCA curve sm2p256v1).
//!
//! Thin chat-skill wrapper around the pure core; chat schema single-sourced from
//! descriptor() (which also drives the CLI); handler delegates to run_skill.
//! Deriving the public point is a single scalar multiplication (Q = d·G), so it
//! is deterministic (no RNG) and runs on every backend. Surfaces: chat + CLI +
//! page. The private key is used only to compute the public point; it is never
//! echoed back in the output.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_sm2_public_from_private_core::derive_formatted;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    private_key: String,
    #[serde(default = "default_input_format")]
    input_format: String,
    #[serde(default = "default_output_format")]
    output_format: String,
}
fn default_input_format() -> String {
    "auto".to_string()
}
fn default_output_format() -> String {
    "all".to_string()
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("private_key").required().describe(
                "The SM2 private key. Either a raw 32-byte private scalar in hexadecimal (64 hex chars, an optional leading 0x is accepted) or a PKCS#8 PEM block (-----BEGIN PRIVATE KEY-----). The scalar must be in [1, n-1] for the sm2p256v1 curve.",
            ),
        )
        .param(
            Param::enumv("input_format", ["auto", "hex", "pem"]).default("auto").describe(
                "How to interpret private_key: 'auto' (default) detects a PEM block, otherwise parses raw hex; 'hex' forces a raw 32-byte hex scalar; 'pem' forces PKCS#8 PEM.",
            ),
        )
        .param(
            Param::enumv("output_format", ["all", "uncompressed", "compressed", "pem"])
                .default("all")
                .describe(
                    "Which encoding of the derived public key to return: 'all' (default) a labelled summary of every encoding; 'uncompressed' SEC1 uncompressed hex (04 || x || y, 130 chars); 'compressed' SEC1 compressed hex (02|03 || x, 66 chars); 'pem' SPKI PEM (-----BEGIN PUBLIC KEY-----).",
                ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Sm2PublicFromPrivate;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/sm2-public-from-private",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Derive the SM2 public key from a private key",
    skill(
        description = "Derive the SM2 public key from a private key (Chinese national standard GM/T 0003, OSCCA curve sm2p256v1). Supply private_key as a raw 32-byte private scalar in hex (input_format=hex) or a PKCS#8 PEM (input_format=pem); input_format=auto (default) detects which. The public point Q = d·G is computed locally and returned per output_format: 'all' (default) a summary of every encoding, 'uncompressed' SEC1 hex (04 || x || y), 'compressed' SEC1 hex (02|03 || x), or 'pem' SPKI PEM. Deterministic, no RNG. The private key is never echoed back — only public key material is returned. To create a fresh SM2 key pair instead, use sm2-keypair-generate.",
        parameters = schema_json()
    ),
)]
impl Sm2PublicFromPrivate {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "sm2-public-from-private", |a: Args| {
            derive_formatted(&a.private_key, &a.input_format, &a.output_format)
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

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema, so any future change to the LLM-facing API is intentional.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "private_key": { "type": "string", "description": "The SM2 private key. Either a raw 32-byte private scalar in hexadecimal (64 hex chars, an optional leading 0x is accepted) or a PKCS#8 PEM block (-----BEGIN PRIVATE KEY-----). The scalar must be in [1, n-1] for the sm2p256v1 curve." },
                    "input_format": { "type": "string", "enum": ["auto", "hex", "pem"], "default": "auto", "description": "How to interpret private_key: 'auto' (default) detects a PEM block, otherwise parses raw hex; 'hex' forces a raw 32-byte hex scalar; 'pem' forces PKCS#8 PEM." },
                    "output_format": { "type": "string", "enum": ["all", "uncompressed", "compressed", "pem"], "default": "all", "description": "Which encoding of the derived public key to return: 'all' (default) a labelled summary of every encoding; 'uncompressed' SEC1 uncompressed hex (04 || x || y, 130 chars); 'compressed' SEC1 compressed hex (02|03 || x, 66 chars); 'pem' SPKI PEM (-----BEGIN PUBLIC KEY-----)." }
                },
                "required": ["private_key"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
