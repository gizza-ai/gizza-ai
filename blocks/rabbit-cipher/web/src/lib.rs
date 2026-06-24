//! Browser-facing wasm-bindgen wrapper for /tools/rabbit-cipher/.
//! Field order MUST match meta.toml: data, operation, key, iv, key_format, format.
use gizza_ai_rabbit_cipher_core::{decrypt, encrypt, Encoding, KeyFormat};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    data: &str,
    operation: &str,
    key: &str,
    iv: &str,
    key_format: &str,
    format: &str,
) -> Result<String, JsValue> {
    let kf = KeyFormat::parse(key_format).map_err(|e| JsValue::from_str(&e))?;
    let fmt = Encoding::parse(format).map_err(|e| JsValue::from_str(&e))?;
    let r = match operation.trim().to_ascii_lowercase().as_str() {
        "decrypt" => decrypt(data, key, iv, kf, fmt),
        _ => encrypt(data, key, iv, kf, fmt),
    };
    r.map_err(|e| JsValue::from_str(&e))
}
