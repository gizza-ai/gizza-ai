//! Browser-facing wasm-bindgen wrapper for /tools/bip39-seed-derive/.
//! Compiled with wasm-pack for the standalone page. The page passes every field
//! value as a string. `mnemonic` is the pasted phrase; `passphrase` is the
//! optional BIP39 25th word (blank = none).
use gizza_ai_bip39_seed_derive_core as bip39;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(mnemonic: &str, passphrase: &str) -> Result<String, JsValue> {
    let s = bip39::derive(mnemonic, passphrase).map_err(|e| JsValue::from_str(&e))?;
    let pass = if s.passphrase.is_empty() {
        "(none)".to_string()
    } else {
        s.passphrase.clone()
    };
    Ok(format!(
        "BIP39 seed (512-bit, hex):\n{}\n\nMnemonic ({} words, valid checksum):\n{}\n\nRecovered entropy ({} bits): {}\nPassphrase: {}",
        s.seed_hex, s.word_count, s.mnemonic, s.strength, s.entropy_hex, pass
    ))
}
