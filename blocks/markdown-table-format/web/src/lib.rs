//! Browser-facing wasm-bindgen wrapper for /tools/markdown-table-format/.
//! Field order MUST match meta.toml: markdown, align, style. Fields are strings.
use gizza_ai_markdown_table_format_core::format_tables_styled;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(markdown: &str, align: &str, style: &str) -> Result<String, JsValue> {
    let a = if align.trim().is_empty() { "keep" } else { align.trim() };
    let s = if style.trim().is_empty() { "pretty" } else { style.trim() };
    format_tables_styled(markdown, a, s).map_err(|e| JsValue::from_str(&e))
}
