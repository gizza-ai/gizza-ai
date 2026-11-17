//! Browser-facing wasm-bindgen wrapper for /tools/xsalsa20-cipher/.

use gizza_ai_xsalsa20_cipher_core::{decrypt, encrypt, Encoding, KeyFormat};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(operation: &str, data: &str, key: &str, nonce: &str, key_format: &str, counter: &str, format: &str) -> Result<String, JsValue> {
    let fmt = Encoding::parse(format).map_err(|e| JsValue::from_str(&e))?;
    let kf = KeyFormat::parse(key_format).map_err(|e| JsValue::from_str(&e))?;
    let counter = counter.trim().parse::<u64>().unwrap_or(0);
    match operation.trim().to_ascii_lowercase().as_str() {
        "" | "encrypt" => encrypt(data, key, nonce, kf, counter, fmt),
        "decrypt" => decrypt(data, key, nonce, kf, counter, fmt),
        other => Err(format!("unknown operation '{other}' (use encrypt or decrypt)")),
    }.map_err(|e| JsValue::from_str(&e))
}
