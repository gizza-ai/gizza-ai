//! Browser-facing wasm-bindgen wrapper for /tools/text-to-table/.
//! Field order MUST match meta.toml: data, format, header, delimiter, align.
use gizza_ai_text_to_table_core::{to_table, Align, Format};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(data: &str, format: &str, header: &str, delimiter: &str, align: &str) -> Result<String, JsValue> {
    let fmt = Format::parse(format).map_err(|e| JsValue::from_str(&e))?;
    let al = Align::parse(align).map_err(|e| JsValue::from_str(&e))?;
    let hdr = !matches!(header.trim().to_ascii_lowercase().as_str(), "false" | "0" | "off" | "no");
    let delim = if delimiter.is_empty() { "," } else { delimiter };
    to_table(data, fmt, hdr, delim, al).map_err(|e| JsValue::from_str(&e))
}
