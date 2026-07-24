//! Browser-facing wasm-bindgen wrapper for /tools/markdown-query/.
//! Field order MUST match meta.toml: markdown, extract, format, include_line_numbers.
use gizza_ai_markdown_query_core::{parse_extract, parse_format, query};
use wasm_bindgen::prelude::*;

/// A checkbox posts "true"/"false" (or "on"/"off"); anything truthy → true.
fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

#[wasm_bindgen]
pub fn run(
    markdown: &str,
    extract: &str,
    format: &str,
    include_line_numbers: &str,
) -> Result<String, JsValue> {
    let extract = parse_extract(extract).map_err(|e| JsValue::from_str(&e))?;
    let format = parse_format(format).map_err(|e| JsValue::from_str(&e))?;
    query(markdown, extract, format, truthy(include_line_numbers))
        .map_err(|e| JsValue::from_str(&e))
}
