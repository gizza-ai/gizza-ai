//! Browser-facing wasm-bindgen wrapper for /tools/markdown-section-extractor/.
//! Compiled with wasm-pack for the standalone page.
use wasm_bindgen::prelude::*;

/// Extract a Markdown section by heading.
///
/// The standalone tool page passes every field value as a string, so the boolean
/// params arrive as strings and are parsed here:
/// - `markdown`: the full document.
/// - `heading`: the heading text to find.
/// - `match_mode`: `"exact"` / `"exact_case"` / `"contains"` (blank → exact).
/// - `include_subsections`: `"true"`/`"1"`/`"yes"`/`"on"` → keep nested
///   subsections; the checkbox defaults to checked (true).
/// - `include_heading`: same truthy parsing; defaults to checked (true).
///
/// Throws a JS error string on a blank heading, an invalid match mode, or no
/// matching heading.
#[wasm_bindgen]
pub fn run(
    markdown: &str,
    heading: &str,
    match_mode: &str,
    include_subsections: &str,
    include_heading: &str,
) -> Result<String, JsValue> {
    let include_subsections = truthy(include_subsections);
    let include_heading = truthy(include_heading);
    gizza_ai_markdown_section_extractor_core::extract(
        markdown,
        heading,
        match_mode,
        include_subsections,
        include_heading,
    )
    .map_err(|e| JsValue::from_str(&e))
}

fn truthy(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    )
}
