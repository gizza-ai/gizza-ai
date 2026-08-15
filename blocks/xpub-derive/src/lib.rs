//! gizza-ai/xpub-derive — watch-only address derivation from an extended public key.
//! The chat schema is single-sourced from descriptor() (which also drives the CLI);
//! handle() delegates to block_utils::run_skill and the shared core.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    xpub: String,
    #[serde(default = "default_chain")]
    chain: String,
    #[serde(default = "default_count")]
    count: u32,
    #[serde(default)]
    start: u32,
    #[serde(default = "default_address_type")]
    address_type: String,
    #[serde(default = "default_format")]
    format: String,
    #[serde(default)]
    include_public_key: bool,
}

fn default_chain() -> String {
    "both".to_string()
}
fn default_count() -> u32 {
    gizza_ai_xpub_derive_core::DEFAULT_COUNT
}
fn default_address_type() -> String {
    "auto".to_string()
}
fn default_format() -> String {
    "table".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("xpub").required().describe(
            "Extended PUBLIC key to derive from, usually at account level (m/44'|49'|84'/coin'/account'). Accepts xpub (legacy), ypub (wrapped SegWit), zpub (native SegWit) on mainnet and tpub/upub/vpub on testnet. Extended private keys (xprv/yprv/zprv) are rejected.",
        ))
        .param(
            Param::enumv("chain", ["receive", "change", "both"])
                .default("both")
                .describe(
                    "Which BIP44 chain to walk below the key: 'receive' = external m/0/i, 'change' = internal m/1/i, 'both' (default) lists each in turn.",
                ),
        )
        .param(
            Param::integer("count")
                .default(10)
                .min(1.0)
                .max(100.0)
                .describe("How many addresses to derive per chain, 1-100. Default 10; 20 matches the usual wallet gap limit."),
        )
        .param(
            Param::integer("start")
                .default(0)
                .min(0.0)
                .max(2147483647.0)
                .describe("First child index to derive, 0-2147483647. Default 0. Use it to page through a chain, e.g. start=20 count=20 for the second batch."),
        )
        .param(
            Param::enumv("address_type", ["auto", "p2pkh", "p2sh_p2wpkh", "p2wpkh"])
                .default("auto")
                .describe(
                    "Address format to render. 'auto' (default) follows the key prefix: xpub/tpub to legacy p2pkh, ypub/upub to wrapped SegWit p2sh_p2wpkh, zpub/vpub to native SegWit p2wpkh. Set it explicitly to render a different format from the same key material.",
                ),
        )
        .param(
            Param::enumv("format", ["table", "csv", "list"])
                .default("table")
                .describe(
                    "Output shape: 'table' (default) adds a key summary plus one padded path/address row per line, 'csv' emits chain,index,path,address rows for spreadsheet import, 'list' emits bare addresses one per line.",
                ),
        )
        .param(
            Param::boolean("include_public_key")
                .default(false)
                .describe("Add the 33-byte compressed public key (hex) as an extra column in table and csv output. Ignored by the list format."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/xpub-derive",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Derive watch-only Bitcoin addresses from an extended public key",
    skill(
        description = "Derive receive and change Bitcoin addresses from an extended PUBLIC key (xpub/ypub/zpub on mainnet, tpub/upub/vpub on testnet) for watch-only inspection. Walks the non-hardened children m/0/i (receive) and m/1/i (change), picks the address format from the key prefix unless overridden, and returns a table, CSV, or bare address list. No private key is accepted or produced; everything runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "xpub-derive", |a: Args| {
            gizza_ai_xpub_derive_core::derive(
                &a.xpub,
                &a.chain,
                a.count,
                a.start,
                &a.address_type,
                &a.format,
                a.include_public_key,
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
                "xpub": { "type": "string", "description": "Extended PUBLIC key to derive from, usually at account level (m/44'|49'|84'/coin'/account'). Accepts xpub (legacy), ypub (wrapped SegWit), zpub (native SegWit) on mainnet and tpub/upub/vpub on testnet. Extended private keys (xprv/yprv/zprv) are rejected." },
                "chain": { "type": "string", "enum": ["receive", "change", "both"], "default": "both", "description": "Which BIP44 chain to walk below the key: 'receive' = external m/0/i, 'change' = internal m/1/i, 'both' (default) lists each in turn." },
                "count": { "type": "integer", "minimum": 1, "maximum": 100, "default": 10, "description": "How many addresses to derive per chain, 1-100. Default 10; 20 matches the usual wallet gap limit." },
                "start": { "type": "integer", "minimum": 0, "maximum": 2147483647, "default": 0, "description": "First child index to derive, 0-2147483647. Default 0. Use it to page through a chain, e.g. start=20 count=20 for the second batch." },
                "address_type": { "type": "string", "enum": ["auto", "p2pkh", "p2sh_p2wpkh", "p2wpkh"], "default": "auto", "description": "Address format to render. 'auto' (default) follows the key prefix: xpub/tpub to legacy p2pkh, ypub/upub to wrapped SegWit p2sh_p2wpkh, zpub/vpub to native SegWit p2wpkh. Set it explicitly to render a different format from the same key material." },
                "format": { "type": "string", "enum": ["table", "csv", "list"], "default": "table", "description": "Output shape: 'table' (default) adds a key summary plus one padded path/address row per line, 'csv' emits chain,index,path,address rows for spreadsheet import, 'list' emits bare addresses one per line." },
                "include_public_key": { "type": "boolean", "default": false, "description": "Add the 33-byte compressed public key (hex) as an extra column in table and csv output. Ignored by the list format." }
            },
            "required": ["xpub"]
        });
        assert_eq!(got, expected);
    }
}
