//! Browser-facing wasm-bindgen wrapper for /tools/csv-to-xml/.
//! Field order MUST match meta.toml: data, root, row, header, delimiter.
use gizza_ai_csv_to_xml_core::to_xml;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(data: &str, root: &str, row: &str, header: &str, delimiter: &str) -> Result<String, JsValue> {
    let hdr = !matches!(header.trim().to_ascii_lowercase().as_str(), "false" | "0" | "off" | "no");
    let root = if root.is_empty() { "rows" } else { root };
    let row = if row.is_empty() { "row" } else { row };
    let delim = if delimiter.is_empty() { "," } else { delimiter };
    to_xml(data, root, row, hdr, delim).map_err(|e| JsValue::from_str(&e))
}
