//! Browser-facing wasm-bindgen wrapper for /tools/unit-converter/.
//! Field order MUST match meta.toml: value, from, to. Fields arrive as strings.
use gizza_ai_unit_converter_core::convert_to_string;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(value: &str, from: &str, to: &str) -> Result<String, JsValue> {
    let v: f64 = value
        .trim()
        .parse()
        .map_err(|_| JsValue::from_str("value must be a number"))?;
    convert_to_string(v, from, to).map_err(|e| JsValue::from_str(&e))
}
