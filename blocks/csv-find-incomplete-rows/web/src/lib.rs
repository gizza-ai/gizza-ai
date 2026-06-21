//! Browser-facing wasm-bindgen wrapper for /tools/csv-find-incomplete-rows/.
//! Field order MUST match meta.toml: data, header, delimiter, required.
use gizza_ai_csv_find_incomplete_rows_core::summary;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(data: &str, header: &str, delimiter: &str, required: &str) -> Result<String, JsValue> {
    let hdr = !matches!(header.trim().to_ascii_lowercase().as_str(), "false" | "0" | "off" | "no");
    let delim = if delimiter.is_empty() { "," } else { delimiter };
    summary(data, hdr, delim, required).map_err(|e| JsValue::from_str(&e))
}
