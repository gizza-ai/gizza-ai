//! gizza-ai/secp256k1-pubkey-derive — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. Pure compute — no host
//! calls — so it runs entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    key: String,
    #[serde(default)]
    format: String,
}

/// Single source for the chat schema (and CLI + page query-params).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("key")
                .required()
                .describe("The secp256k1 private key: 64 hex characters (0x prefix, spaces, and underscores allowed) or a WIF (base58check, e.g. 5.../K.../L... mainnet, 9.../c... testnet). The WIF's network and compression flag don't affect the public-key point and are ignored."),
        )
        .param(
            Param::enumv("format", ["all", "compressed", "uncompressed", "x", "y"])
                .default("all")
                .describe("Which representation to return. 'all' (default) lists every field; 'compressed' = 33-byte SEC1 point (02/03 prefix); 'uncompressed' = 65-byte SEC1 point (04 prefix); 'x' = the 32-byte X coordinate (also the Taproot x-only key); 'y' = the 32-byte Y coordinate. Single formats return the bare hex value."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/secp256k1-pubkey-derive",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Derive the secp256k1 public key from a private key",
    skill(
        description = "Derive the secp256k1 public key from a private key. Give key= as 64-char hex (0x/spaces/underscores allowed) or a WIF; the tool computes pubkey = key·G and returns the compressed SEC1 point (33 bytes, 02/03 prefix), the uncompressed point (65 bytes, 04 prefix), the raw X coordinate (32 bytes, also the Taproot x-only key), the raw Y coordinate, and the Y parity. The point is chain-agnostic (Bitcoin/Ethereum/Tron/...). Set format= to compressed, uncompressed, x, or y to return just that bare hex value; default 'all' lists everything. This tool derives from a GIVEN key — it does not generate random keys or emit addresses.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned value in { "result": ... }.
        match run_skill(&body, "secp256k1-pubkey-derive", |a: Args| {
            gizza_ai_secp256k1_pubkey_derive_core::derive(&a.key, &a.format)
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
                    "key": { "type": "string", "description": "The secp256k1 private key: 64 hex characters (0x prefix, spaces, and underscores allowed) or a WIF (base58check, e.g. 5.../K.../L... mainnet, 9.../c... testnet). The WIF's network and compression flag don't affect the public-key point and are ignored." },
                    "format": { "type": "string", "enum": ["all", "compressed", "uncompressed", "x", "y"], "default": "all", "description": "Which representation to return. 'all' (default) lists every field; 'compressed' = 33-byte SEC1 point (02/03 prefix); 'uncompressed' = 65-byte SEC1 point (04 prefix); 'x' = the 32-byte X coordinate (also the Taproot x-only key); 'y' = the 32-byte Y coordinate. Single formats return the bare hex value." }
                },
                "required": ["key"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
