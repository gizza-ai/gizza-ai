//! Browser-facing wasm-bindgen wrapper for /tools/chacha20-cipher/.
//! Field order MUST match meta.toml: data, operation, key, nonce, aad, mode,
//! key_format, counter, format.
use gizza_ai_chacha20_cipher_core::{decrypt, encrypt, Encoding, KeyFormat, Mode};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    data: &str,
    operation: &str,
    key: &str,
    nonce: &str,
    aad: &str,
    mode: &str,
    key_format: &str,
    counter: &str,
    format: &str,
) -> Result<String, JsValue> {
    let kf = KeyFormat::parse(key_format).map_err(|e| JsValue::from_str(&e))?;
    let m = Mode::parse(mode).map_err(|e| JsValue::from_str(&e))?;
    let fmt = Encoding::parse(format).map_err(|e| JsValue::from_str(&e))?;
    let ctr = if counter.trim().is_empty() {
        0
    } else {
        counter
            .trim()
            .parse::<u32>()
            .map_err(|_| JsValue::from_str("counter must be a non-negative whole number"))?
    };
    let r = match operation.trim().to_ascii_lowercase().as_str() {
        "decrypt" => decrypt(data, key, nonce, aad, kf, m, ctr, fmt),
        _ => encrypt(data, key, nonce, aad, kf, m, ctr, fmt),
    };
    r.map_err(|e| JsValue::from_str(&e))
}
