//! Browser-facing wasm-bindgen wrapper for /tools/extract-domains/.
//! Field order MUST match meta.toml: text, mode, sort. Fields arrive as strings.
use gizza_ai_extract_domains_core::render;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(text: &str, mode: &str, sort: &str) -> Result<String, JsValue> {
    let sort = matches!(
        sort.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    );
    render(text, mode, sort).map_err(|e| JsValue::from_str(&e))
}
