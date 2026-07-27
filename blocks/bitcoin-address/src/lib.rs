//! gizza-ai/bitcoin-address — derive Bitcoin P2PKH, P2SH-P2WPKH, P2WPKH and WIF from a secp256k1 private key.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    key: String,
    #[serde(default = "default_network")]
    network: String,
    #[serde(default = "default_compressed")]
    compressed: bool,
}

fn default_network() -> String {
    "mainnet".to_string()
}
fn default_compressed() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("key")
                .required()
                .describe("Bitcoin secp256k1 private key as 64 hex characters (0x prefix, spaces, and underscores allowed) or WIF. WIF input carries its own network and compression flag."),
        )
        .param(
            Param::enumv("network", ["mainnet", "testnet"])
                .default("mainnet")
                .describe("Bitcoin network for hex private-key input: mainnet uses 1/3/bc1 address prefixes and 0x80 WIF; testnet uses m/n/2/tb1 and 0xef WIF. Ignored when key is WIF."),
        )
        .param(
            Param::boolean("compressed")
                .default(true)
                .describe("Use the compressed 33-byte public key for P2PKH and WIF when key is hex. SegWit P2SH-P2WPKH and P2WPKH are only emitted for compressed keys. Ignored when key is WIF."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct BitcoinAddress;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/bitcoin-address",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Derive Bitcoin addresses and WIF from a private key",
    skill(
        description = "Derive Bitcoin addresses and WIF from an existing secp256k1 private key. Accepts a 32-byte private key as hex or WIF, derives the compressed or uncompressed public key, and returns the private key hex, WIF, public key hex, HASH160, legacy P2PKH, wrapped SegWit P2SH-P2WPKH, and native SegWit P2WPKH addresses. Supports mainnet and testnet for hex input; WIF input auto-detects its own network and compression flag. Runs locally with pure Rust SHA-256, RIPEMD-160, Base58Check, and Bech32 logic.",
        parameters = schema_json()
    ),
)]
impl BitcoinAddress {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "bitcoin-address", |a: Args| {
            gizza_ai_bitcoin_address_core::derive(&a.key, &a.network, a.compressed)
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
                    "key": { "type": "string", "description": "Bitcoin secp256k1 private key as 64 hex characters (0x prefix, spaces, and underscores allowed) or WIF. WIF input carries its own network and compression flag." },
                    "network": { "type": "string", "enum": ["mainnet", "testnet"], "default": "mainnet", "description": "Bitcoin network for hex private-key input: mainnet uses 1/3/bc1 address prefixes and 0x80 WIF; testnet uses m/n/2/tb1 and 0xef WIF. Ignored when key is WIF." },
                    "compressed": { "type": "boolean", "default": true, "description": "Use the compressed 33-byte public key for P2PKH and WIF when key is hex. SegWit P2SH-P2WPKH and P2WPKH are only emitted for compressed keys. Ignored when key is WIF." }
                },
                "required": ["key"],
                "additionalProperties": false
            }"#,
        ).unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
