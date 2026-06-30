//! Browser-facing wasm-bindgen wrapper for /tools/rc2-cipher/.
//! Field order MUST match meta.toml: data, operation, cipher, key, iv,
//! effective_key_bits, format. The page passes every field as a string, so
//! effective_key_bits is parsed here (blank/invalid → 0 = use full key length).
use gizza_ai_rc2_cipher_core::{decrypt, encrypt, Encoding, Mode};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    data: &str,
    operation: &str,
    cipher: &str,
    key: &str,
    iv: &str,
    effective_key_bits: &str,
    format: &str,
) -> Result<String, JsValue> {
    let mode = Mode::parse(cipher).map_err(|e| JsValue::from_str(&e))?;
    let fmt = Encoding::parse(format).map_err(|e| JsValue::from_str(&e))?;
    let eff: u32 = effective_key_bits.trim().parse().unwrap_or(0);
    let r = match operation.trim().to_ascii_lowercase().as_str() {
        "decrypt" => decrypt(data, key, iv, eff, mode, fmt),
        _ => encrypt(data, key, iv, eff, mode, fmt),
    };
    r.map_err(|e| JsValue::from_str(&e))
}
