//! Browser-facing wasm-bindgen wrapper for /tools/html-entity-encoder/.
//! `tool.js` passes every field value as a raw string, so `scope` and `format`
//! arrive as `&str` and the core parses them (blank → defaults minimal/named).
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(text: &str, scope: &str, format: &str) -> Result<String, JsValue> {
    gizza_ai_html_entity_encoder_core::encode(text, scope, format).map_err(|e| JsValue::from_str(&e))
}
