//! Browser-facing wasm-bindgen wrapper for /tools/morse-code/.
//! Compiled with wasm-pack for the standalone /tools/morse-code/ page.
use wasm_bindgen::prelude::*;

/// Convert `text` to or from Morse code.
///
/// The standalone tool page passes every field value as a string:
/// - `direction`: `"encode"`/`"decode"` (blank → encode).
/// - `letter_sep`: separator between letters (blank → a single space).
/// - `word_sep`: separator between words (blank → " / ").
///
/// Throws a JS error string on an invalid `direction` or an unrecognised Morse
/// token while decoding.
#[wasm_bindgen]
pub fn run(
    text: &str,
    direction: &str,
    letter_sep: &str,
    word_sep: &str,
) -> Result<String, JsValue> {
    gizza_ai_morse_code_core::convert(text, direction, letter_sep, word_sep)
        .map_err(|e| JsValue::from_str(&e))
}
