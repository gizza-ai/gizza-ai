//! Browser-facing wasm-bindgen wrapper for /tools/json-yaml-converter/.
//! Field order MUST match meta.toml: input, direction, pretty.
use gizza_ai_json_yaml_converter_core::{convert, resolve_direction};
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
pub fn run(input: &str, direction: &str, pretty: &str) -> Result<String, JsValue> {
    let dir = resolve_direction(direction, input).map_err(|e| JsValue::from_str(&e))?;
    convert(input, dir, truthy(pretty)).map_err(|e| JsValue::from_str(&e))
}
