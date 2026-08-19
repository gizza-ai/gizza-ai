//! Browser-facing wasm-bindgen wrapper for /tools/stopword-filter/.
//! Field order MUST match meta.toml: text, language, custom_words, keep_words,
//! case_sensitive, remove_punctuation, output.
use gizza_ai_stopword_filter_core::filter_text;
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

#[wasm_bindgen]
pub fn run(
    text: &str,
    language: &str,
    custom_words: &str,
    keep_words: &str,
    case_sensitive: &str,
    remove_punctuation: &str,
    output: &str,
) -> Result<String, JsValue> {
    filter_text(
        text,
        language,
        custom_words,
        keep_words,
        truthy(case_sensitive),
        truthy(remove_punctuation),
        output,
    )
    .map_err(|e| JsValue::from_str(&e))
}
