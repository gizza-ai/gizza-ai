//! Browser-facing wasm-bindgen wrapper for /tools/unwrap-text/.
//! Field order MUST match meta.toml: text, keep_list_breaks.
use gizza_ai_unwrap_text_core::unwrap;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(text: &str, keep_list_breaks: &str) -> Result<String, JsValue> {
    let keep = !matches!(keep_list_breaks.trim().to_ascii_lowercase().as_str(), "false" | "0" | "no" | "off");
    Ok(unwrap(text, keep))
}
