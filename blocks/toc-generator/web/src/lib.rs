//! Browser-facing wasm-bindgen wrapper for /tools/toc-generator/.
//! Compiled with wasm-pack for the standalone page.
use wasm_bindgen::prelude::*;

/// Build a table of contents from a Markdown or HTML document.
///
/// The standalone tool page passes every field value as a string:
/// - `document`: the Markdown or HTML source.
/// - `input_format`: `"auto"`/`"markdown"`/`"html"` (blank → auto-detect).
/// - `output_format`: `"markdown"`/`"html"` (blank → markdown).
/// - `min_level`/`max_level`: heading levels 1-6 (blank/unparseable → 1 / 6).
/// - `ordered`: `"true"`/`"1"`/`"on"`/`"yes"` → numbered list (default false).
///
/// Throws a JS error string on empty input, an invalid format, or no headings.
#[wasm_bindgen]
pub fn run(
    document: &str,
    input_format: &str,
    output_format: &str,
    min_level: &str,
    max_level: &str,
    ordered: &str,
) -> Result<String, JsValue> {
    let min_level = min_level.trim().parse::<u32>().unwrap_or(1);
    let max_level = max_level.trim().parse::<u32>().unwrap_or(6);
    let ordered = matches!(
        ordered.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    );
    gizza_ai_toc_generator_core::generate(
        document,
        input_format,
        output_format,
        min_level,
        max_level,
        ordered,
    )
    .map_err(|e| JsValue::from_str(&e))
}
