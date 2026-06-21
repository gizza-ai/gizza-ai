//! Browser-facing wasm-bindgen wrapper for /tools/rsa-sign/.
//! Field order MUST match meta.toml: message, private_key, scheme, hash.
use gizza_ai_rsa_sign_core::{sign, Hash, Scheme};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(message: &str, private_key: &str, scheme: &str, hash: &str) -> Result<String, JsValue> {
    let scheme = Scheme::parse(scheme).map_err(|e| JsValue::from_str(&e))?;
    let hash = Hash::parse(hash).map_err(|e| JsValue::from_str(&e))?;
    sign(message, private_key, scheme, hash).map_err(|e| JsValue::from_str(&e))
}
