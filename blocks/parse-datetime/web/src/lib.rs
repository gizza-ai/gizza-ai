//! Browser-facing wasm-bindgen wrapper for /tools/parse-datetime/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(input: &str) -> Result<String, JsValue> {
    gizza_ai_parse_datetime_core::render(input).map_err(|e| JsValue::from_str(&e))
}
