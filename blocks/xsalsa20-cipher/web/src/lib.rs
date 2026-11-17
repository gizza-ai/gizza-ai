//! Browser-facing wasm-bindgen wrapper for /tools/xsalsa20-cipher/.

use gizza_ai_xsalsa20_cipher_core::{decrypt, encrypt, Encoding, KeyFormat};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(operation: &str, data: &str, key: &str, nonce: &str, key_format: &str, counter: &str, format: &str) -> Result<String, JsValue> {
    let fmt = Encoding::parse(format).map_err(|e| JsValue::from_str(&e))?;
    let kf = KeyFormat::parse(key_format).map_err(|e| JsValue::from_str(&e))?;
    let counter = match counter.trim() {
        "" => 0,
        c => c.parse::<u64>().map_err(|_| {
            JsValue::from_str(if c.bytes().all(|b| b.is_ascii_digit()) {
                "counter too large: the maximum is 18446744073709551615"
            } else {
                "counter must be a non-negative whole number"
            })
        })?,
    };
    match operation.trim().to_ascii_lowercase().as_str() {
        "" | "encrypt" => encrypt(data, key, nonce, kf, counter, fmt),
        "decrypt" => decrypt(data, key, nonce, kf, counter, fmt),
        other => Err(format!("unknown operation '{other}' (use encrypt or decrypt)")),
    }.map_err(|e| JsValue::from_str(&e))
}
