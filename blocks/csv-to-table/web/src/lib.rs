//! Browser-facing wasm-bindgen wrapper for /tools/csv-to-table/.
//! Field order MUST match meta.toml: data, format, header, delimiter.
use gizza_ai_csv_to_table_core::{to_table, Format};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(data: &str, format: &str, header: &str, delimiter: &str) -> Result<String, JsValue> {
    let fmt = Format::parse(format).map_err(|e| JsValue::from_str(&e))?;
    let hdr = !matches!(header.trim().to_ascii_lowercase().as_str(), "false" | "0" | "off" | "no");
    let delim = if delimiter.is_empty() { "," } else { delimiter };
    to_table(data, fmt, hdr, delim).map_err(|e| JsValue::from_str(&e))
}
