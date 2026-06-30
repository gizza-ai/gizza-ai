//! Browser-facing wasm-bindgen wrapper for /tools/unicode-to-text/.
//! Field order MUST match meta.toml: text.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(text: &str) -> Result<String, JsValue> {
    gizza_ai_unicode_to_text_core::decode(text).map_err(|e| JsValue::from_str(&e))
}
