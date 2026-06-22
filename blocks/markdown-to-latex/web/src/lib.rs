//! Browser-facing wasm-bindgen wrapper for /tools/markdown-to-latex/.
//! Compiled with wasm-pack for the standalone /tools/markdown-to-latex/ page.
use wasm_bindgen::prelude::*;

/// Convert `markdown` to LaTeX.
///
/// The standalone tool page passes every field value as a string, so the
/// boolean/integer params arrive as strings and are parsed here:
/// - `full_document`: `"true"`/`"1"`/`"yes"`/`"on"` → wrap in a standalone
///   document; anything else (including blank) → emit only the body fragment.
/// - `heading_offset`: a count `0`–5 (blank/unparseable → 0; the core clamps).
///
/// Throws a JS error string only on an internal conversion failure (the core is
/// otherwise infallible — malformed Markdown degrades to literal text).
#[wasm_bindgen]
pub fn run(markdown: &str, full_document: &str, heading_offset: &str) -> Result<String, JsValue> {
    let full_document = matches!(
        full_document.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    );
    let heading_offset = heading_offset.trim().parse::<u32>().unwrap_or(0);
    gizza_ai_markdown_to_latex_core::convert(markdown, full_document, heading_offset)
        .map_err(|e| JsValue::from_str(&e))
}
