//! Browser-facing wasm-bindgen wrapper for /tools/cidr-calculator/.
//! Compiled with wasm-pack for the standalone /tools/cidr-calculator/ page.
use wasm_bindgen::prelude::*;

/// Compute the subnet report for a CIDR. The page passes each field as a string;
/// a blank `format` falls back to the core default ("text"). Throws a JS error
/// string on invalid input.
#[wasm_bindgen]
pub fn run(input: &str, format: &str) -> Result<String, JsValue> {
    gizza_ai_cidr_calculator_core::calculate(input, format).map_err(|e| JsValue::from_str(&e))
}
