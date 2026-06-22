//! Browser-facing wasm-bindgen wrapper for /tools/csv-change-delimiter/.
use gizza_ai_csv_change_delimiter_core::change_delimiter;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(data: &str, from: &str, to: &str) -> Result<String, JsValue> {
    let from = if from.is_empty() { "," } else { from };
    let to = if to.is_empty() { "tab" } else { to };
    change_delimiter(data, from, to).map_err(|e| JsValue::from_str(&e))
}
