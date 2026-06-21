//! Browser-facing wasm-bindgen wrapper for /tools/parse-uri/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(uri: &str) -> Result<String, JsValue> {
    gizza_ai_parse_uri_core::render(uri).map_err(|e| JsValue::from_str(&e))
}
