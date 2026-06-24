//! Browser-facing wasm-bindgen wrapper for /tools/parse-tls-record/.
use gizza_ai_parse_tls_record_core::render;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(record: &str) -> Result<String, JsValue> {
    render(record).map_err(|e| JsValue::from_str(&e))
}
