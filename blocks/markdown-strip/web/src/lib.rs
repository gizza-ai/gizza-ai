//! Browser-facing wasm-bindgen wrapper for /tools/markdown-strip/.
//! Compiled with wasm-pack for the standalone /tools/markdown-strip/ page.
use wasm_bindgen::prelude::*;

/// Return true for the checkbox "true"/"1"/"yes"/"on" values, false otherwise.
fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    )
}

/// Strip Markdown formatting from `text`, returning clean plain text.
///
/// The standalone tool page passes every field value as a string, so the
/// enum/boolean params arrive as strings:
/// - `links`: `"text"` (default) / `"url"` / `"both"`.
/// - `images`: `"alt"` (default) / `"drop"`.
/// - `keep_list_markers`: `"true"`/`"false"` — keep list bullets/numbering.
/// - `collapse_blank_lines`: `"true"`/`"false"` — single vs blank-line spacing.
///
/// Throws a JS error string on an invalid `links`/`images` value or empty input.
#[wasm_bindgen]
pub fn run(
    text: &str,
    links: &str,
    images: &str,
    keep_list_markers: &str,
    collapse_blank_lines: &str,
) -> Result<String, JsValue> {
    gizza_ai_markdown_strip_core::strip(
        text,
        links,
        images,
        truthy(keep_list_markers),
        truthy(collapse_blank_lines),
    )
    .map_err(|e| JsValue::from_str(&e))
}
