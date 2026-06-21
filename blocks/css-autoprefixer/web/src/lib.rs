//! Browser-facing wasm-bindgen wrapper for /tools/css-autoprefixer/.
//! Compiled with wasm-pack for the standalone /tools/css-autoprefixer/ page.
use wasm_bindgen::prelude::*;

/// Add vendor prefixes to `css`.
///
/// The standalone tool page passes every field value as a string, so the boolean
/// `dedup` arrives as a string and is parsed here: `"true"`/`"1"`/`"on"`/`"yes"`
/// (the default-checked box) → dedup on; anything else → off.
///
/// Throws a JS error string when the input CSS is empty.
#[wasm_bindgen]
pub fn run(css: &str, dedup: &str) -> Result<String, JsValue> {
    let dedup = matches!(
        dedup.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    );
    gizza_ai_css_autoprefixer_core::prefix(css, dedup).map_err(|e| JsValue::from_str(&e))
}
