//! Browser-facing wasm-bindgen wrapper for /tools/html-table-extractor/.
//! Field order MUST match meta.toml: html, format, table_index, header.
use gizza_ai_html_table_extractor_core::{extract, parse_format};
use wasm_bindgen::prelude::*;

fn truthy_default_true(s: &str) -> bool {
    !matches!(s.trim().to_ascii_lowercase().as_str(), "false" | "0" | "off" | "no")
}

#[wasm_bindgen]
pub fn run(html: &str, format: &str, table_index: &str, header: &str) -> Result<String, JsValue> {
    let idx: usize = table_index.trim().parse().unwrap_or(0);
    let fmt = parse_format(format).map_err(|e| JsValue::from_str(&e))?;
    extract(html, idx, fmt, truthy_default_true(header)).map_err(|e| JsValue::from_str(&e))
}
