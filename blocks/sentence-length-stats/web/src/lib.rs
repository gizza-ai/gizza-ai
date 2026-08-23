//! Browser-facing wasm-bindgen wrapper for /tools/sentence-length-stats/.
//! Field order must match page/meta.toml: text, newlines, long_threshold,
//! list_longest, extra_abbreviations.
use wasm_bindgen::prelude::*;

fn parse_usize(value: &str, field: &str, default: usize) -> Result<usize, JsValue> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(default);
    }
    trimmed.parse::<usize>().map_err(|_| {
        JsValue::from_str(&format!("{field} must be a whole number (got {trimmed:?})"))
    })
}

#[wasm_bindgen]
pub fn run(
    text: &str,
    newlines: &str,
    long_threshold: &str,
    list_longest: &str,
    extra_abbreviations: &str,
) -> Result<String, JsValue> {
    gizza_ai_sentence_length_stats_core::run(
        text,
        if newlines.trim().is_empty() {
            "paragraph"
        } else {
            newlines
        },
        parse_usize(long_threshold, "long_threshold", 25)?,
        parse_usize(list_longest, "list_longest", 3)?,
        extra_abbreviations,
    )
    .map_err(|e| JsValue::from_str(&e))
}
