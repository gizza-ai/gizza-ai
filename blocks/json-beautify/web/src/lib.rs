//! Browser-facing wasm-bindgen wrapper for /tools/json-beautify/.
//! Field order MUST match meta.toml: json, indent. Fields arrive as strings.
use gizza_ai_json_beautify_core::format;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(json: &str, indent: &str) -> Result<String, JsValue> {
    let n: usize = indent.trim().parse().unwrap_or(2);
    format(json, n).map_err(|e| JsValue::from_str(&e))
}
