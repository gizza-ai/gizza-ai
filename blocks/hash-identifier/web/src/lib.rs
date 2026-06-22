//! Browser-facing wasm-bindgen wrapper for /tools/hash-identifier/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(input: &str) -> Result<String, JsValue> {
    gizza_ai_hash_identifier_core::run(input).map_err(|e| JsValue::from_str(&e))
}
