//! Browser-facing wasm-bindgen wrapper for /tools/multi-encoder/.
//! Field order MUST match meta.toml: text, encoding, direction.
use gizza_ai_multi_encoder_core::{parse_direction, parse_encoding, transform};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(text: &str, encoding: &str, direction: &str) -> Result<String, JsValue> {
    let enc = parse_encoding(encoding).map_err(|e| JsValue::from_str(&e))?;
    let dir = parse_direction(direction).map_err(|e| JsValue::from_str(&e))?;
    transform(text, enc, dir).map_err(|e| JsValue::from_str(&e))
}
