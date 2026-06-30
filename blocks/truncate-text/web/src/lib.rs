//! Browser-facing wasm-bindgen wrapper for /tools/truncate-text/.
//! Compiled with wasm-pack for the standalone page.
use gizza_ai_truncate_text_core::Unit;
use wasm_bindgen::prelude::*;

/// Shorten `text` to `length` characters or words, appending `ellipsis`.
///
/// The standalone tool page passes every field value as a string:
/// - `length`: a unit count (blank/unparseable → 100; the core clamps the range).
/// - `unit`: "characters" or "words" (the page renders a `<select>`).
/// - `count_ellipsis` / `break_words`: `"true"`/`"1"`/`"on"`/`"yes"` → on; anything
///   else (including blank) → off. The page renders these as checkboxes whose
///   default-checked state comes from the descriptor.
///
/// Throws a JS error string when `length` is out of range or `unit` is invalid.
#[wasm_bindgen]
pub fn run(
    text: &str,
    length: &str,
    unit: &str,
    ellipsis: &str,
    count_ellipsis: &str,
    break_words: &str,
) -> Result<String, JsValue> {
    let length = length.trim().parse::<u32>().unwrap_or(100);
    let unit = Unit::parse(unit).map_err(|e| JsValue::from_str(&e))?;
    // On the page a blank ellipsis field is ambiguous, so fall back to the
    // default marker. (The chat/CLI surface passes the literal value, so an
    // explicit empty string there means "hard cut, no marker".)
    let ellipsis = if ellipsis.is_empty() { "…" } else { ellipsis };
    let truthy =
        |v: &str| matches!(v.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes");
    gizza_ai_truncate_text_core::truncate(
        text,
        length,
        unit,
        ellipsis,
        truthy(count_ellipsis),
        truthy(break_words),
    )
    .map_err(|e| JsValue::from_str(&e))
}
