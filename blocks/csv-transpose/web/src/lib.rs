//! Browser-facing wasm-bindgen wrapper for /tools/csv-transpose/.
//! Field order MUST match meta.toml: data, delimiter.
use gizza_ai_csv_transpose_core::transpose;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(data: &str, delimiter: &str) -> Result<String, JsValue> {
    let delim = if delimiter.is_empty() { "," } else { delimiter };
    transpose(data, delim).map_err(|e| JsValue::from_str(&e))
}
