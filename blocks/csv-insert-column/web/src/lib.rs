//! Browser-facing wasm-bindgen wrapper for /tools/csv-insert-column/.
use gizza_ai_csv_insert_column_core::insert_column;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(data: &str, name: &str, value: &str, position: &str, header: &str, delimiter: &str) -> Result<String, JsValue> {
    let hdr = !matches!(header.trim().to_ascii_lowercase().as_str(), "false" | "0" | "off" | "no");
    let pos = if position.is_empty() { "end" } else { position };
    let delim = if delimiter.is_empty() { "," } else { delimiter };
    insert_column(data, name, value, pos, hdr, delim).map_err(|e| JsValue::from_str(&e))
}
