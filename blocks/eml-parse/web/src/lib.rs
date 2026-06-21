//! Browser-facing wasm-bindgen wrapper for /tools/eml-parse/.
use gizza_ai_eml_parse_core::render;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(eml: &str) -> Result<String, JsValue> {
    render(eml).map_err(|e| JsValue::from_str(&e))
}
