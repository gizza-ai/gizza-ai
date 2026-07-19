//! Browser-facing wasm-bindgen wrapper for /tools/har-validator/.
//! Field order MUST match meta.toml: har, check_timings. Fields arrive as strings.
use gizza_ai_har_validator_core::validate;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(har: &str, check_timings: &str) -> Result<String, JsValue> {
    // Default-true checkbox: empty (deep-link without the param) → on;
    // only an explicit falsey value turns it off.
    let check = !matches!(check_timings.trim(), "false" | "0" | "no" | "off");
    validate(har, check).map_err(|e| JsValue::from_str(&e))
}
