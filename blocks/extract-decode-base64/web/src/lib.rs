//! Browser-facing wasm-bindgen wrapper for /tools/extract-decode-base64/.
use gizza_ai_extract_decode_base64_core::render;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(text: &str) -> Result<String, JsValue> {
    render(text).map_err(|e| JsValue::from_str(&e))
}
