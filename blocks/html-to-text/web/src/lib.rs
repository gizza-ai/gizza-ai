//! Browser-facing wasm-bindgen wrapper for /tools/html-to-text/.
use gizza_ai_html_to_text_core::convert;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(html: &str) -> Result<String, JsValue> {
    convert(html).map_err(|e| JsValue::from_str(&e))
}
