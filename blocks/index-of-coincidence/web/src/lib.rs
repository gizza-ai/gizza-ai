//! Browser-facing wasm-bindgen wrapper for /tools/index-of-coincidence/.
//! Compiled with wasm-pack for the standalone /tools/index-of-coincidence/ page.
//!
//! The page passes every field value as a string, so `max_period` and
//! `show_counts` arrive as strings and are parsed here.
use wasm_bindgen::prelude::*;

/// Compute the Index of Coincidence report for `text`.
///
/// - `max_period`: a count `0`–40 (blank/unparseable → 0; the core clamps the
///   range). `> 0` adds Vigenère key-length estimation.
/// - `show_counts`: `"true"`/`"1"`/`"yes"`/`"on"` → include the per-letter table.
///
/// Throws a JS error string when the text contains no A-Z letters.
#[wasm_bindgen]
pub fn run(text: &str, max_period: &str, show_counts: &str) -> Result<String, JsValue> {
    let max_period = max_period.trim().parse::<u32>().unwrap_or(0);
    let show_counts = matches!(
        show_counts.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    );
    gizza_ai_index_of_coincidence_core::report(text, max_period, show_counts)
        .map_err(|e| JsValue::from_str(&e))
}
