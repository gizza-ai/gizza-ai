//! Browser-facing wasm-bindgen wrapper for /tools/hashtag-extractor/.
//! Field order MUST match page/meta.toml: text, max_tags, platform, style,
//! phrase_words, min_word_length, include_existing, separator. Each value
//! arrives as a string (checkboxes send "true"/"false"); blank numeric fields
//! fall back to the documented defaults.
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

/// Parse an optional whole-number field: blank → `fallback`.
fn parse_int(s: &str, field: &str, fallback: i64) -> Result<i64, JsValue> {
    let t = s.trim();
    if t.is_empty() {
        return Ok(fallback);
    }
    t.parse::<i64>()
        .map_err(|_| JsValue::from_str(&format!("{field} must be a whole number (got '{t}')")))
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    text: &str,
    max_tags: &str,
    platform: &str,
    style: &str,
    phrase_words: &str,
    min_word_length: &str,
    include_existing: &str,
    separator: &str,
) -> Result<String, JsValue> {
    let max_tags = parse_int(max_tags, "max_tags", 10)?;
    let phrase_words = parse_int(phrase_words, "phrase_words", 1)?;
    let min_word_length = parse_int(min_word_length, "min_word_length", 3)?;
    gizza_ai_hashtag_extractor_core::run(
        text,
        max_tags,
        platform,
        style,
        phrase_words,
        min_word_length,
        truthy(include_existing),
        separator,
    )
    .map_err(|e| JsValue::from_str(&e))
}
