//! Browser-facing wasm-bindgen wrapper for /tools/http-header-analyzer/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(headers: &str) -> Result<String, JsValue> {
    gizza_ai_http_header_analyzer_core::render(headers).map_err(|e| JsValue::from_str(&e))
}
