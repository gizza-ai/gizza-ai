//! Browser-facing wasm-bindgen wrapper for /tools/strip-udp-header/.
use gizza_ai_strip_udp_header_core::render;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(datagram: &str) -> Result<String, JsValue> {
    render(datagram).map_err(|e| JsValue::from_str(&e))
}
