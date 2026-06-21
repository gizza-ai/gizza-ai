//! Browser-facing wasm-bindgen wrapper for /tools/strip-ipv4-header/.
use gizza_ai_strip_ipv4_header_core::render;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(packet: &str) -> Result<String, JsValue> {
    render(packet).map_err(|e| JsValue::from_str(&e))
}
