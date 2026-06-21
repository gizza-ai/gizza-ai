//! Browser-facing wasm-bindgen wrapper for /tools/textrank-summarize/.
//! Field order MUST match meta.toml: text, sentences.
use gizza_ai_textrank_summarize_core::summarize;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(text: &str, sentences: &str) -> Result<String, JsValue> {
    let n: usize = sentences.trim().parse().unwrap_or(3).max(1);
    Ok(summarize(text, n))
}
