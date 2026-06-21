//! Browser-facing wasm-bindgen wrapper for /tools/extract-ip-addresses/.
use gizza_ai_extract_ip_addresses_core::render;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(text: &str) -> Result<String, JsValue> {
    render(text).map_err(|e| JsValue::from_str(&e))
}
