//! Browser-facing wasm-bindgen wrapper for /tools/outline-to-json/.
//! Compiled with wasm-pack for the standalone page.
use wasm_bindgen::prelude::*;

/// Convert an indented outline into JSON.
///
/// The standalone tool page passes every field value as a string:
/// - `outline`: the indented text outline.
/// - `format`: `"children"`/`"nested"` (blank → children).
/// - `tab_size`: a count (blank/unparseable → 4; the core clamps 1..=16).
/// - `pretty`: `"true"`/`"1"`/`"on"`/`"yes"` → pretty (default true; anything
///   else, including `"false"`, → compact).
///
/// Throws a JS error string on an invalid `format` or empty/invalid outline.
#[wasm_bindgen]
pub fn run(outline: &str, format: &str, tab_size: &str, pretty: &str) -> Result<String, JsValue> {
    let tab_size = tab_size.trim().parse::<u32>().unwrap_or(4);
    // Default-true checkbox: blank means the box is still default-checked.
    let pretty = matches!(
        pretty.trim().to_ascii_lowercase().as_str(),
        "" | "true" | "1" | "on" | "yes"
    );
    gizza_ai_outline_to_json_core::convert(outline, format, tab_size, pretty)
        .map_err(|e| JsValue::from_str(&e))
}
