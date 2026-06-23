//! Browser-facing wasm-bindgen wrapper for /tools/ja3-fingerprint/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(client_hello: &str) -> Result<String, JsValue> {
    gizza_ai_ja3_fingerprint_core::render(client_hello).map_err(|e| JsValue::from_str(&e))
}
