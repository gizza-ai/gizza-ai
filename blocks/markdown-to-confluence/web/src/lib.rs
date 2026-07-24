//! Browser-facing wasm-bindgen wrapper for /tools/markdown-to-confluence/.
//! Compiled with wasm-pack for the standalone /tools/markdown-to-confluence/ page.
use wasm_bindgen::prelude::*;

/// Convert `input` Markdown to Confluence markup.
///
/// The standalone tool page passes every field value as a string, so the
/// enum/boolean/integer params arrive as strings and are parsed here:
/// - `format`: `"storage"` (default) or `"wiki"`; blank → `"storage"`.
/// - `panel_blockquotes`: a checkbox → `"true"`/`"false"`; blank falls back to
///   the default (`true`); only an explicit falsey value turns it off.
/// - `heading_offset`: a count `0`–5 (blank/unparseable → 0; the core clamps).
///
/// Throws a JS error string on empty input or an unknown format (the core is
/// otherwise infallible — malformed Markdown degrades to literal text).
#[wasm_bindgen]
pub fn run(
    input: &str,
    format: &str,
    panel_blockquotes: &str,
    heading_offset: &str,
) -> Result<String, JsValue> {
    let format = if format.trim().is_empty() { "storage" } else { format };
    let panel_blockquotes = !matches!(
        panel_blockquotes.trim().to_ascii_lowercase().as_str(),
        "false" | "0" | "no" | "off"
    );
    let heading_offset = heading_offset.trim().parse::<u32>().unwrap_or(0);
    gizza_ai_markdown_to_confluence_core::convert(input, format, panel_blockquotes, heading_offset)
        .map_err(|e| JsValue::from_str(&e))
}
