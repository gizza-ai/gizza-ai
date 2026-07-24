//! Browser-facing wasm-bindgen wrapper for /tools/html-entity-decoder/.
//! `tool.js` passes every field value as a raw string, so `unknown` arrives as a
//! `&str` and the core parses it (blank → default 'keep').
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(text: &str, unknown: &str) -> Result<String, JsValue> {
    gizza_ai_html_entity_decoder_core::decode(text, unknown).map_err(|e| JsValue::from_str(&e))
}
