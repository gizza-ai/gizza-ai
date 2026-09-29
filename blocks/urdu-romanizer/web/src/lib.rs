//! Browser-facing wasm-bindgen wrapper for /tools/urdu-romanizer/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    input: &str,
    scheme: &str,
    short_vowels: &str,
    common_words: &str,
    digits: &str,
    punctuation: &str,
    capitalization: &str,
) -> Result<String, JsValue> {
    let common_words = matches!(common_words, "true" | "1" | "on" | "yes");
    gizza_ai_urdu_romanizer_core::run(
        input,
        scheme,
        short_vowels,
        common_words,
        digits,
        punctuation,
        capitalization,
    )
    .map_err(|e| JsValue::from_str(&e))
}
