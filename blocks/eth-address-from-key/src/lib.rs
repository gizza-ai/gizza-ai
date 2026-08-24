//! gizza-ai/eth-address-from-key — chat skill block on the shared tool abstraction.
//! Derives Ethereum address forms from secp256k1 private or public keys.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    key: String,
    #[serde(default = "default_key_type")]
    key_type: String,
    #[serde(default = "default_output_format")]
    output_format: String,
}

fn default_key_type() -> String {
    "auto".to_string()
}

fn default_output_format() -> String {
    "all".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("key").required().describe(
            "A secp256k1 key as hex: a 32-byte private key (64 hex chars), a 33-byte compressed public key, a 65-byte uncompressed SEC1 public key, or a raw 64-byte x||y public key. A leading 0x plus spaces, underscores, colons and hyphens are ignored.",
        ))
        .param(
            Param::enumv("key_type", ["auto", "private", "public"])
                .default("auto")
                .describe(
                    "How to interpret key. auto (default) treats 32 bytes as a private key and 33/64/65 bytes as a public key; private requires a secret scalar; public requires compressed, uncompressed, or x||y public-key bytes.",
                ),
        )
        .param(
            Param::enumv("output_format", ["all", "checksum", "lowercase", "no-prefix", "json"])
                .default("all")
                .describe(
                    "Which address form to return. all prints the EIP-55 checksum address, lowercase address, bare 40-hex address, and compressed/uncompressed public keys; checksum returns only the EIP-55 0x address; lowercase returns the all-lowercase 0x address; no-prefix returns the bare 40 hex characters; json returns the same fields as JSON.",
                ),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/eth-address-from-key",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Derive Ethereum address and EIP-55 checksum forms from secp256k1 keys.",
    skill(
        description = "Derive an Ethereum address from a secp256k1 private key or public key. The tool accepts 32-byte private-key hex, compressed or uncompressed SEC1 public-key hex, or raw x||y public-key hex; computes Keccak-256 over the uncompressed public key without its 0x04 prefix; and returns the EIP-55 checksum address plus lowercase and no-prefix forms.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "eth-address-from-key", |a: Args| {
            gizza_ai_eth_address_from_key_core::run(&a.key, &a.key_type, &a.output_format)
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
    fn schema_matches_contract() {
        let actual: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let expected: serde_json::Value = serde_json::json!({
            "type": "object",
            "required": ["key"],
            "properties": {
                "key": { "type": "string", "description": actual["properties"]["key"]["description"] },
                "key_type": {
                    "type": "string",
                    "enum": ["auto", "private", "public"],
                    "default": "auto",
                    "description": actual["properties"]["key_type"]["description"],
                },
                "output_format": {
                    "type": "string",
                    "enum": ["all", "checksum", "lowercase", "no-prefix", "json"],
                    "default": "all",
                    "description": actual["properties"]["output_format"]["description"],
                },
            },
            "additionalProperties": false,
        });
        assert_eq!(actual, expected);
    }

    #[test]
    fn every_param_is_described() {
        let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        for (name, prop) in schema["properties"].as_object().unwrap() {
            let desc = prop["description"].as_str().unwrap_or("");
            assert!(desc.len() > 30, "param {name} needs a real description");
        }
    }
}
