//! Browser-facing wasm-bindgen wrapper for /tools/word-frequency/.
//! Field order MUST match meta.toml: text, case_sensitive, min_length,
//! ignore_stopwords, top.
use gizza_ai_word_frequency_core::format_table;
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

fn num(s: &str, default: usize) -> usize {
    s.trim().parse::<usize>().unwrap_or(default)
}

#[wasm_bindgen]
pub fn run(
    text: &str,
    case_sensitive: &str,
    min_length: &str,
    ignore_stopwords: &str,
    top: &str,
) -> Result<String, JsValue> {
    Ok(format_table(
        text,
        truthy(case_sensitive),
        num(min_length, 1),
        truthy(ignore_stopwords),
        num(top, 0),
    ))
}
