//! Browser-facing wasm-bindgen wrapper for /tools/extract-urls/.
//! Field order MUST match meta.toml: text, split_components. Fields arrive as strings.
use gizza_ai_extract_urls_core::render;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(text: &str, split_components: &str) -> Result<String, JsValue> {
    let split = matches!(
        split_components.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    );
    render(text, split).map_err(|e| JsValue::from_str(&e))
}
