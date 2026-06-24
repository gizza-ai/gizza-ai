//! Browser-facing wasm-bindgen wrapper for /tools/tweet-thread-splitter/.
//! Compiled with wasm-pack for the standalone /tools/tweet-thread-splitter/ page.
use wasm_bindgen::prelude::*;

/// Split `text` into a numbered tweet thread, returned as plain text with the
/// tweets separated by a blank line.
///
/// The standalone tool page passes every field value as a string, so the
/// integer/boolean params arrive as strings and are parsed here:
/// - `limit`: max chars per tweet (blank/unparseable → 280; core clamps the range).
/// - `numbering`: `"parens"` (blank → parens) | `"slash"` | `"dotted"` | `"none"`.
/// - `count`: `"chars"` (blank → chars) or `"utf16"`.
/// - `prefer_sentences`: `"true"`/`"1"`/`"yes"`/`"on"` → on; anything else → off.
///   (The page renders this checkbox checked by default, so a normal load sends `"true"`.)
///
/// Throws a JS error string on an invalid `numbering`/`count`, an out-of-range
/// `limit`, or empty input.
#[wasm_bindgen]
pub fn run(
    text: &str,
    limit: &str,
    numbering: &str,
    count: &str,
    prefer_sentences: &str,
) -> Result<String, JsValue> {
    let limit = limit.trim().parse::<usize>().unwrap_or(0);
    let prefer_sentences = matches!(
        prefer_sentences.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    );
    gizza_ai_tweet_thread_splitter_core::split(text, limit, numbering, count, prefer_sentences)
        .map(|t| t.to_plain())
        .map_err(|e| JsValue::from_str(&e))
}
