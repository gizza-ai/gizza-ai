//! Browser-facing wasm-bindgen wrapper for /tools/bip39-mnemonic-generator/.
//! Compiled with wasm-pack for the standalone page. The page passes every field
//! value as a string. `strength` is the bit count ("128".."256"); `entropy_hex`
//! blank → secure random; `passphrase` is the optional BIP39 25th word.
use gizza_ai_bip39_mnemonic_generator_core as bip39;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(strength: &str, entropy_hex: &str, passphrase: &str) -> Result<String, JsValue> {
    let m = if entropy_hex.trim().is_empty() {
        let bits = strength.trim().parse::<usize>().unwrap_or(128);
        bip39::generate(bits, passphrase)
    } else {
        bip39::from_entropy_hex(entropy_hex, passphrase)
    }
    .map_err(|e| JsValue::from_str(&e))?;

    let pass = if m.passphrase.is_empty() {
        "(none)".to_string()
    } else {
        m.passphrase.clone()
    };
    Ok(format!(
        "Mnemonic ({} words):\n{}\n\nEntropy ({} bits): {}\nPassphrase: {}\nBIP39 seed (512-bit, hex):\n{}",
        m.word_count, m.mnemonic, m.strength, m.entropy_hex, pass, m.seed_hex
    ))
}
