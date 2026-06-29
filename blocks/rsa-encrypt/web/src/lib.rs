//! Browser-facing wasm-bindgen wrapper for /tools/rsa-encrypt/.
//! Field order MUST match meta.toml: message, public_key, padding, hash.
use gizza_ai_rsa_encrypt_core::{encrypt, Hash, Padding};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(message: &str, public_key: &str, padding: &str, hash: &str) -> Result<String, JsValue> {
    let padding = Padding::parse(padding).map_err(|e| JsValue::from_str(&e))?;
    let hash = Hash::parse(hash).map_err(|e| JsValue::from_str(&e))?;
    encrypt(message, public_key, padding, hash).map_err(|e| JsValue::from_str(&e))
}
