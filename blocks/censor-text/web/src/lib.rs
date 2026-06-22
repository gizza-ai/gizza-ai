//! Browser-facing wasm-bindgen wrapper for /tools/censor-text/.
use gizza_ai_censor_text_core::censor;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(text: &str, words: &str, mask: &str, whole_word: &str) -> Result<String, JsValue> {
    let ww = !matches!(whole_word.trim().to_ascii_lowercase().as_str(), "false" | "0" | "off" | "no");
    censor(text, words, mask, ww).map_err(|e| JsValue::from_str(&e))
}
