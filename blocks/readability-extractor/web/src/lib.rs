//! Browser-facing wasm-bindgen wrapper for /tools/readability-extractor/.
use gizza_ai_readability_extractor_core::extract;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(html: &str, format: &str) -> Result<String, JsValue> {
    let as_html = format.trim().eq_ignore_ascii_case("html");
    extract(html, as_html).map_err(|e| JsValue::from_str(&e))
}
