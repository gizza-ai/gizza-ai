//! Browser-facing wasm-bindgen wrapper for /tools/csv-dedupe/.
use gizza_ai_csv_dedupe_core::dedupe;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(data: &str, columns: &str, header: &str, delimiter: &str) -> Result<String, JsValue> {
    let hdr = !matches!(header.trim().to_ascii_lowercase().as_str(), "false" | "0" | "off" | "no");
    let delim = if delimiter.is_empty() { "," } else { delimiter };
    dedupe(data, columns, hdr, delim).map_err(|e| JsValue::from_str(&e))
}
