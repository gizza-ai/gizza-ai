//! Browser-facing wasm-bindgen wrapper for /tools/json-diff/.
//! Field order MUST match meta.toml: left, right, indent.
use gizza_ai_json_diff_core::diff_documents;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(left: &str, right: &str, indent: &str) -> Result<String, JsValue> {
    let n: usize = indent.trim().parse().unwrap_or(2);
    diff_documents(left, right, n).map_err(|e| JsValue::from_str(&e))
}
