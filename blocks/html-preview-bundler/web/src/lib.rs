//! Browser-facing wasm-bindgen wrapper for /tools/html-preview-bundler/.
//! Field order MUST match meta.toml: html, css, js, title.
use gizza_ai_html_preview_bundler_core::bundle;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(html: &str, css: &str, js: &str, title: &str) -> Result<String, JsValue> {
    bundle(html, css, js, title).map_err(|e| JsValue::from_str(&e))
}
