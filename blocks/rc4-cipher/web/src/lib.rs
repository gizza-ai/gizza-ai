//! Browser-facing wasm-bindgen wrapper for /tools/rc4-cipher/.
//! Field order MUST match meta.toml: data, operation, key, key_format, drop, format.
use gizza_ai_rc4_cipher_core::{decrypt, encrypt, Encoding, KeyFormat};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    data: &str,
    operation: &str,
    key: &str,
    key_format: &str,
    drop: &str,
    format: &str,
) -> Result<String, JsValue> {
    let kf = KeyFormat::parse(key_format).map_err(|e| JsValue::from_str(&e))?;
    let fmt = Encoding::parse(format).map_err(|e| JsValue::from_str(&e))?;
    let drop_n = if drop.trim().is_empty() {
        0
    } else {
        drop.trim()
            .parse::<usize>()
            .map_err(|_| JsValue::from_str("drop must be a non-negative whole number"))?
    };
    let r = match operation.trim().to_ascii_lowercase().as_str() {
        "decrypt" => decrypt(data, key, kf, drop_n, fmt),
        _ => encrypt(data, key, kf, drop_n, fmt),
    };
    r.map_err(|e| JsValue::from_str(&e))
}
