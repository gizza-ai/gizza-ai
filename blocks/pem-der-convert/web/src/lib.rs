//! Browser-facing wasm-bindgen wrapper for /tools/pem-der-convert/.
//! Field order MUST match meta.toml: input, direction, der_format, label.
use gizza_ai_pem_der_convert_core::{parse_der_format, parse_direction, run};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run_convert(
    input: &str,
    direction: &str,
    der_format: &str,
    label: &str,
) -> Result<String, JsValue> {
    let dir = parse_direction(direction).map_err(|e| JsValue::from_str(&e))?;
    let fmt = parse_der_format(der_format).map_err(|e| JsValue::from_str(&e))?;
    run(input, dir, fmt, label).map_err(|e| JsValue::from_str(&e))
}
