//! Browser-facing wasm-bindgen wrapper for /tools/csv-filter/.
use gizza_ai_csv_filter_core::filter;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(data: &str, condition: &str, header: &str, delimiter: &str) -> Result<String, JsValue> {
    let hdr = !matches!(header.trim().to_ascii_lowercase().as_str(), "false" | "0" | "off" | "no");
    let delim = if delimiter.is_empty() { "," } else { delimiter };
    filter(data, condition, hdr, delim).map_err(|e| JsValue::from_str(&e))
}
