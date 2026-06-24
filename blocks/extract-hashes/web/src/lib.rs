//! Browser-facing wasm-bindgen wrapper for /tools/extract-hashes/.
//! Field order MUST match meta.toml: text, lowercase. Fields arrive as strings.
use gizza_ai_extract_hashes_core::render;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(text: &str, lowercase: &str) -> Result<String, JsValue> {
    let lower = matches!(
        lowercase.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    );
    render(text, lower).map_err(|e| JsValue::from_str(&e))
}
