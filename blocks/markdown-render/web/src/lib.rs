//! Browser-facing wasm-bindgen wrapper for /tools/markdown-render/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(markdown: &str) -> Result<String, JsValue> {
    gizza_ai_markdown_render_core::render(markdown).map_err(|e| JsValue::from_str(&e))
}
