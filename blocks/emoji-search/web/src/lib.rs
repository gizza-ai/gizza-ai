//! Browser-facing wasm-bindgen wrapper for /tools/emoji-search/.
//! Compiled with wasm-pack for the standalone /tools/emoji-search/ page.
use wasm_bindgen::prelude::*;

/// Search the embedded emoji dataset.
///
/// The standalone tool page passes every field value as a string:
/// - `query`: keyword / `:shortcode:` / category (required, non-empty).
/// - `limit`: a count `1`–100 (blank/unparseable → 20; the core clamps).
/// - `glyphs_only`: `"true"`/`"1"`/`"yes"`/`"on"` → bare glyphs; else labelled.
///
/// Throws a JS error string when the query is empty.
#[wasm_bindgen]
pub fn run(query: &str, limit: &str, glyphs_only: &str) -> Result<String, JsValue> {
    let limit = limit.trim().parse::<u32>().unwrap_or(0);
    let glyphs_only = matches!(
        glyphs_only.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    );
    let res = if glyphs_only {
        gizza_ai_emoji_search_core::glyphs(query, limit)
    } else {
        gizza_ai_emoji_search_core::run(query, limit)
    };
    res.map_err(|e| JsValue::from_str(&e))
}
