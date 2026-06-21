//! Browser-facing wasm-bindgen wrapper for /tools/html-formatter/.
//! Field order MUST match meta.toml: html, indent. Fields arrive as strings.
use gizza_ai_html_formatter_core::format;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(html: &str, indent: &str) -> Result<String, JsValue> {
    let n: usize = indent.trim().parse().unwrap_or(2);
    format(html, n).map_err(|e| JsValue::from_str(&e))
}
