//! Browser-facing wasm-bindgen wrapper for /tools/secp256k1-pubkey-derive/.
//! The page passes every field value as a string (field order in page/meta.toml:
//! key, format), so `run` takes `&str` args and delegates to the core.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(key: &str, format: &str) -> Result<String, JsValue> {
    gizza_ai_secp256k1_pubkey_derive_core::derive(key, format).map_err(|e| JsValue::from_str(&e))
}
