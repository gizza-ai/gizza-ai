//! Browser-facing wasm-bindgen wrapper for /tools/csv-reorder-columns/.
//! Field order MUST match meta.toml: data, columns, header, delimiter.
use gizza_ai_csv_reorder_columns_core::reorder;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(data: &str, columns: &str, header: &str, delimiter: &str) -> Result<String, JsValue> {
    let hdr = !matches!(header.trim().to_ascii_lowercase().as_str(), "false" | "0" | "off" | "no");
    let delim = if delimiter.is_empty() { "," } else { delimiter };
    reorder(data, columns, hdr, delim).map_err(|e| JsValue::from_str(&e))
}
