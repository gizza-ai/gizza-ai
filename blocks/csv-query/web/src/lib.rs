//! Browser-facing wasm-bindgen wrapper for /tools/csv-query/.
use gizza_ai_csv_query_core::query;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(data: &str, q: &str, delimiter: &str) -> Result<String, JsValue> {
    let delim = if delimiter.is_empty() { "," } else { delimiter };
    query(data, q, delim).map_err(|e| JsValue::from_str(&e))
}
