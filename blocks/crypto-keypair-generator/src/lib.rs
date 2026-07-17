//! gizza-ai/crypto-keypair-generator — chat skill block on the shared tool
//! abstraction. The chat schema is single-sourced from descriptor() (which also
//! drives the CLI); handle() delegates to block_utils::run_skill. Pure Rust
//! (getrandom CSPRNG) → runs on all backends. Surfaces: chat + CLI. No
//! standalone page — key generation is non-deterministic, which doesn't fit the
//! page's live-recompute-on-input model.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_crypto_keypair_generator_core::{generate, Chain};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    #[serde(default = "default_chain")]
    chain: String,
}
fn default_chain() -> String {
    "bitcoin".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None).param(
        Param::enumv("chain", ["bitcoin", "ethereum", "solana"])
            .default("bitcoin")
            .describe(
                "Blockchain to generate a keypair for: bitcoin (secp256k1, legacy P2PKH address + WIF), ethereum (secp256k1, EIP-55 address), or solana (Ed25519, base58 address). Default bitcoin.",
            ),
    )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct CryptoKeypairGenerator;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/crypto-keypair-generator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Generate a crypto wallet keypair + address",
    skill(
        description = "Generate a fresh cryptographic keypair and wallet address for a blockchain — bitcoin, ethereum, or solana — entirely offline. Returns the private key (hex, plus the chain-native export: Bitcoin WIF, Ethereum 0x-hex, or Solana base58 keypair), the public key (hex), and the wallet address in the chain's canonical encoding (Bitcoin legacy P2PKH base58check, Ethereum EIP-55 checksummed hex, or Solana base58). Keys are generated locally with a cryptographic RNG; nothing is sent anywhere. Default chain is bitcoin.",
        parameters = schema_json()
    ),
)]
impl CryptoKeypairGenerator {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "crypto-keypair-generator", |a: Args| {
            let chain = Chain::parse(&a.chain).map_err(SkillError::InvalidArgs)?;
            Ok(generate(chain))
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
                    "chain": { "type": "string", "enum": ["bitcoin", "ethereum", "solana"], "default": "bitcoin", "description": "Blockchain to generate a keypair for: bitcoin (secp256k1, legacy P2PKH address + WIF), ethereum (secp256k1, EIP-55 address), or solana (Ed25519, base58 address). Default bitcoin." }
                },
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
