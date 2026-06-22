//! Browser-facing wasm-bindgen wrapper for /tools/parse-tcp-header/.
use gizza_ai_parse_tcp_header_core::render;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(header: &str) -> Result<String, JsValue> {
    render(header).map_err(|e| JsValue::from_str(&e))
}
