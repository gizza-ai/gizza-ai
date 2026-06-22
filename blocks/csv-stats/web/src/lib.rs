//! Browser-facing wasm-bindgen wrapper for /tools/csv-stats/.
//! Field order MUST match meta.toml: data, header, delimiter.
use gizza_ai_csv_stats_core::summary;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(data: &str, header: &str, delimiter: &str) -> Result<String, JsValue> {
    let hdr = !matches!(header.trim().to_ascii_lowercase().as_str(), "false" | "0" | "off" | "no");
    let delim = if delimiter.is_empty() { "," } else { delimiter };
    summary(data, hdr, delim).map_err(|e| JsValue::from_str(&e))
}
