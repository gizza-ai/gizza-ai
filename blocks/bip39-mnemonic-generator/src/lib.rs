//! gizza-ai/bip39-mnemonic-generator — generate a BIP39 mnemonic seed phrase
//! (12–24 words) from secure entropy, with the BIP39 checksum and the derived
//! 512-bit seed.
//!
//! Thin wrapper around the pure core; chat schema single-sourced from
//! descriptor(); handler delegates to run_skill. Pure Rust (getrandom CSPRNG) →
//! runs on all backends. Surfaces: chat + CLI + page (generation is instant).
//! Supplying `entropy_hex` makes it deterministic (recovery / test vectors).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_bip39_mnemonic_generator_core as bip39;
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize, Default)]
#[serde(default)]
struct Args {
    /// Entropy strength in bits (128/160/192/224/256). Default 128 → 12 words.
    /// Sent as a string by the enum schema / page / CLI.
    strength: String,
    /// Optional explicit entropy as hex (16–32 bytes). Blank → secure random.
    entropy_hex: String,
    /// Optional BIP39 passphrase ("25th word") mixed into the derived seed.
    passphrase: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::enumv("strength", ["128", "160", "192", "224", "256"])
                .default("128")
                .describe("Entropy strength in bits: 128→12 words, 160→15, 192→18, 224→21, 256→24. Default 128. Ignored when entropy_hex is supplied."),
        )
        .param(
            Param::string("entropy_hex")
                .describe("Optional explicit entropy as a hex string of 16/20/24/28/32 bytes (128–256 bits). When set, the mnemonic is derived deterministically from it (for recovery or test vectors); when blank, secure random entropy is used."),
        )
        .param(
            Param::string("passphrase")
                .describe("Optional BIP39 passphrase (the \"25th word\"). It does not change the words but is mixed into the derived 512-bit seed. Leave blank for none."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn run(a: Args) -> Result<bip39::Mnemonic, String> {
    if a.entropy_hex.trim().is_empty() {
        let s = a.strength.trim();
        let strength = if s.is_empty() {
            128
        } else {
            s.parse::<usize>()
                .map_err(|_| format!("strength must be a number, got '{s}'"))?
        };
        bip39::generate(strength, &a.passphrase)
    } else {
        bip39::from_entropy_hex(&a.entropy_hex, &a.passphrase)
    }
}

#[cfg(target_arch = "wasm32")]
struct Bip39MnemonicGenerator;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/bip39-mnemonic-generator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Generate a BIP39 mnemonic seed phrase",
    skill(
        description = "Generate a BIP39 mnemonic seed phrase (12–24 words) from cryptographically secure entropy, with the correct BIP39 checksum word, and derive the 512-bit BIP39 seed. Set strength to 128/160/192/224/256 bits for 12/15/18/21/24 words (default 128 → 12 words). Optionally supply entropy_hex (16–32 bytes of hex) to derive the mnemonic deterministically (recovery / test vectors) instead of random entropy, and a passphrase (the BIP39 \"25th word\") which is mixed into the derived seed. Returns the mnemonic, word count, entropy (hex), and the BIP39 seed (hex). Used by Bitcoin and most HD crypto wallets (BIP32/BIP44).",
        parameters = schema_json()
    ),
)]
impl Bip39MnemonicGenerator {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "bip39-mnemonic-generator", |a: Args| {
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
                    "strength": { "type": "string", "enum": ["128", "160", "192", "224", "256"], "default": "128", "description": "Entropy strength in bits: 128→12 words, 160→15, 192→18, 224→21, 256→24. Default 128. Ignored when entropy_hex is supplied." },
                    "entropy_hex": { "type": "string", "description": "Optional explicit entropy as a hex string of 16/20/24/28/32 bytes (128–256 bits). When set, the mnemonic is derived deterministically from it (for recovery or test vectors); when blank, secure random entropy is used." },
                    "passphrase": { "type": "string", "description": "Optional BIP39 passphrase (the \"25th word\"). It does not change the words but is mixed into the derived 512-bit seed. Leave blank for none." }
                },
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn run_deterministic_from_entropy() {
        let m = run(Args {
            strength: String::new(),
            entropy_hex: "00000000000000000000000000000000".into(),
            passphrase: "TREZOR".into(),
        })
        .unwrap();
        assert_eq!(m.word_count, 12);
        assert!(m.mnemonic.starts_with("abandon abandon"));
    }
}
