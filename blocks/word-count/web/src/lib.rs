//! Browser-facing wasm-bindgen wrapper for /tools/word-count/.
//! Compiled with wasm-pack for the standalone /tools/word-count/ page.
use wasm_bindgen::prelude::*;

/// Count words, characters, and lines in `text`. Throws a JS error string on failure.
#[wasm_bindgen]
pub fn count(text: &str) -> Result<String, JsValue> {
    gizza_ai_word_count_core::count(text).map_err(|e| JsValue::from_str(&e))
}
