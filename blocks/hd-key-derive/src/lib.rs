//! gizza-ai/hd-key-derive — derive BIP32 private child keys and Bitcoin addresses.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    #[serde(default)]
    seed: String,
    #[serde(default)]
    xprv: String,
    path: String,
    #[serde(default = "default_network")]
    network: String,
    #[serde(default = "default_address_type")]
    address_type: String,
}

fn default_network() -> String { "mainnet".to_string() }
fn default_address_type() -> String { "p2pkh".to_string() }

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("seed").describe("Optional BIP32 seed as hex (16–64 bytes, often produced by the bip39-seed-derive tool). Provide exactly one of seed or xprv."))
        .param(Param::string("xprv").describe("Optional BIP32 extended private key (xprv/tprv) to start from instead of a seed. Provide exactly one of seed or xprv."))
        .param(Param::string("path").required().describe("BIP32 derivation path such as m, m/0', m/44'/0'/0'/0/0. Hardened segments may end with ', h, or H."))
        .param(Param::enumv("network", ["mainnet", "testnet"]).default("mainnet").describe("Bitcoin network for version bytes and address prefixes: 'mainnet' (xprv/xpub, 1/3/bc1) or 'testnet' (tprv/tpub, m/n/2/tb1)."))
        .param(Param::enumv("address_type", ["p2pkh", "p2sh_p2wpkh", "p2wpkh"]).default("p2pkh").describe("Address format to render for the derived public key: legacy 'p2pkh', wrapped SegWit 'p2sh_p2wpkh', or native SegWit 'p2wpkh'."))
}
fn schema_json() -> String { descriptor().to_schema_json() }

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/hd-key-derive",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Derive BIP32 child keys and Bitcoin addresses",
    skill(
        description = "Derive a BIP32 hierarchical-deterministic child private key from either a hex seed or an xprv/tprv plus a path such as m/44'/0'/0'/0/0. Returns xprv/xpub, raw private/public keys, WIF, fingerprint, and a Bitcoin mainnet/testnet P2PKH, wrapped SegWit, or native SegWit address. Runs locally; provide exactly one of seed or xprv.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "hd-key-derive", |a: Args| {
            gizza_ai_hd_key_derive_core::derive(
                &a.seed,
                &a.xprv,
                &a.path,
                &a.network,
                &a.address_type,
            )
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
        let got: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let expected = serde_json::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "seed": { "type": "string", "description": "Optional BIP32 seed as hex (16–64 bytes, often produced by the bip39-seed-derive tool). Provide exactly one of seed or xprv." },
                "xprv": { "type": "string", "description": "Optional BIP32 extended private key (xprv/tprv) to start from instead of a seed. Provide exactly one of seed or xprv." },
                "path": { "type": "string", "description": "BIP32 derivation path such as m, m/0', m/44'/0'/0'/0/0. Hardened segments may end with ', h, or H." },
                "network": { "type": "string", "enum": ["mainnet", "testnet"], "default": "mainnet", "description": "Bitcoin network for version bytes and address prefixes: 'mainnet' (xprv/xpub, 1/3/bc1) or 'testnet' (tprv/tpub, m/n/2/tb1)." },
                "address_type": { "type": "string", "enum": ["p2pkh", "p2sh_p2wpkh", "p2wpkh"], "default": "p2pkh", "description": "Address format to render for the derived public key: legacy 'p2pkh', wrapped SegWit 'p2sh_p2wpkh', or native SegWit 'p2wpkh'." }
            },
            "required": ["path"]
        });
        assert_eq!(got, expected);
    }
}
