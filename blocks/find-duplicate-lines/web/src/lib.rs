//! Browser-facing wasm-bindgen wrapper for /tools/find-duplicate-lines/.
//! Field order MUST match meta.toml: text, ignore_case, trim. Fields are strings.
use gizza_ai_find_duplicate_lines_core::render;
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
pub fn run(text: &str, ignore_case: &str, trim: &str) -> Result<String, JsValue> {
    render(text, truthy(ignore_case), truthy(trim)).map_err(|e| JsValue::from_str(&e))
}
