//! Browser-facing wasm-bindgen wrapper for /tools/text-wrap/.
//! Compiled with wasm-pack for the standalone page.
use wasm_bindgen::prelude::*;

/// Hard-wrap `text` to `width` columns.
///
/// The standalone tool page passes every field value as a string, so the
/// numeric/boolean params arrive as strings and are parsed here:
/// - `width`: a column count (blank/unparseable → 80; the core clamps the range).
/// - `preserve_indent` / `break_long_words`: `"true"`/`"1"`/`"on"`/`"yes"` → on;
///   anything else (including blank) → off. The page renders these as checkboxes
///   whose default-checked state comes from the descriptor.
///
/// Throws a JS error string when `width` is out of range.
#[wasm_bindgen]
pub fn run(
    text: &str,
    width: &str,
    preserve_indent: &str,
    break_long_words: &str,
) -> Result<String, JsValue> {
    let width = width.trim().parse::<u32>().unwrap_or(80);
    let truthy =
        |v: &str| matches!(v.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes");
    gizza_ai_text_wrap_core::wrap(text, width, truthy(preserve_indent), truthy(break_long_words))
        .map_err(|e| JsValue::from_str(&e))
}
