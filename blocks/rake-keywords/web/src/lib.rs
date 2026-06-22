//! Browser-facing wasm-bindgen wrapper for /tools/rake-keywords/.
//! Field order MUST match meta.toml: text, top_n, max_words. Fields arrive as strings.
use gizza_ai_rake_keywords_core::render;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(text: &str, top_n: &str, max_words: &str) -> Result<String, JsValue> {
    let top_n = top_n.trim().parse::<usize>().unwrap_or(10);
    let max_words = max_words.trim().parse::<usize>().unwrap_or(0);
    render(text, top_n, max_words).map_err(|e| JsValue::from_str(&e))
}
