//! Browser-facing wasm-bindgen wrapper for /tools/rtf-to-markdown/.
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    )
}

#[wasm_bindgen]
pub fn run(
    rtf: &str,
    headings: &str,
    tables: &str,
    underline: &str,
    links: &str,
    escape_markdown: &str,
) -> Result<String, JsValue> {
    gizza_ai_rtf_to_markdown_core::rtf_to_markdown(
        rtf,
        headings,
        tables,
        underline,
        truthy(links),
        truthy(escape_markdown),
    )
    .map_err(|e| JsValue::from_str(&e))
}
