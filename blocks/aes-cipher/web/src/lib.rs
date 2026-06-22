//! Browser-facing wasm-bindgen wrapper for /tools/aes-cipher/.
//! Field order MUST match meta.toml: data, operation, cipher, key, iv, format.
use gizza_ai_aes_cipher_core::{decrypt, encrypt, Encoding, Mode};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    data: &str,
    operation: &str,
    cipher: &str,
    key: &str,
    iv: &str,
    format: &str,
) -> Result<String, JsValue> {
    let mode = Mode::parse(cipher).map_err(|e| JsValue::from_str(&e))?;
    let fmt = Encoding::parse(format).map_err(|e| JsValue::from_str(&e))?;
    let r = match operation.trim().to_ascii_lowercase().as_str() {
        "decrypt" => decrypt(data, key, iv, mode, fmt),
        _ => encrypt(data, key, iv, mode, fmt),
    };
    r.map_err(|e| JsValue::from_str(&e))
}
