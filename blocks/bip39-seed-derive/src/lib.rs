//! gizza-ai/bip39-seed-derive — derive the BIP39 512-bit seed from an existing
//! (pasted) mnemonic phrase + optional passphrase, validating the wordlist and
//! checksum first. Thin wrapper around the pure core; chat schema single-sourced
//! from descriptor(); handler delegates to run_skill. Pure Rust → runs on all
//! backends. Surfaces: chat + CLI + page.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_bip39_seed_derive_core as bip39;
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize, Default)]
#[serde(default)]
struct Args {
    /// The BIP39 mnemonic phrase to derive from (12/15/18/21/24 words).
    mnemonic: String,
    /// Optional BIP39 passphrase (the "25th word") mixed into the seed.
    passphrase: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("mnemonic")
                .required()
                .describe("The BIP39 mnemonic (recovery) phrase to derive the seed from — 12, 15, 18, 21, or 24 space-separated words from the BIP39 English wordlist. Case and extra whitespace are tolerated. Rejected if a word is unknown or the checksum fails."),
        )
        .param(
            Param::string("passphrase")
                .describe("Optional BIP39 passphrase (the \"25th word\"). It does not change the words but is mixed into the derived 512-bit seed, producing a completely different seed. Leave blank for none."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn run(a: Args) -> Result<bip39::Seed, String> {
    bip39::derive(&a.mnemonic, &a.passphrase)
}

#[cfg(target_arch = "wasm32")]
struct Bip39SeedDerive;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/bip39-seed-derive",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Derive the BIP39 512-bit seed from a mnemonic phrase",
    skill(
        description = "Derive the 512-bit BIP39 seed from an EXISTING mnemonic (recovery) phrase plus an optional passphrase. Validates that the phrase is 12/15/18/21/24 words, every word is in the BIP39 English wordlist, and the checksum passes, then stretches it with PBKDF2-HMAC-SHA512 (2048 iterations) into the master seed that BIP32/BIP44 HD wallets use. Optionally supply a passphrase (the BIP39 \"25th word\"), which does not change the words but yields an entirely different seed. Returns the normalized mnemonic, word count, recovered entropy (hex), and the BIP39 seed (hex). Use bip39-mnemonic-generator to CREATE a new phrase; use this to derive the seed from one you already have.",
        parameters = schema_json()
    ),
)]
impl Bip39SeedDerive {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "bip39-seed-derive", |a: Args| {
            run(a).map_err(SkillError::InvalidArgs)
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
                    "mnemonic": { "type": "string", "description": "The BIP39 mnemonic (recovery) phrase to derive the seed from — 12, 15, 18, 21, or 24 space-separated words from the BIP39 English wordlist. Case and extra whitespace are tolerated. Rejected if a word is unknown or the checksum fails." },
                    "passphrase": { "type": "string", "description": "Optional BIP39 passphrase (the \"25th word\"). It does not change the words but is mixed into the derived 512-bit seed, producing a completely different seed. Leave blank for none." }
                },
                "required": ["mnemonic"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn run_derives_known_vector() {
        let m = run(Args {
            mnemonic: "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about".into(),
            passphrase: "TREZOR".into(),
        })
        .unwrap();
        assert_eq!(m.word_count, 12);
        assert_eq!(
            m.seed_hex,
            "c55257c360c07c72029aebc1b53c05ed0362ada38ead3e3e9efa3708e53495531f09a6987599d18264c1e1c92f2cf141630c7a3c4ab7c81b2f001698e7463b04"
        );
    }

    #[test]
    fn run_rejects_bad_checksum() {
        let e = run(Args {
            mnemonic: "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon".into(),
            passphrase: String::new(),
        })
        .unwrap_err();
        assert!(e.contains("checksum"), "got: {e}");
    }
}
