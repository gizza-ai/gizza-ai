//! Browser-facing wasm-bindgen wrapper for /tools/bitwise-calculator/.
//! Param order (a, op, b, bits) must match page/meta.toml's [[input]] order.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(a: &str, op: &str, b: &str, bits: &str) -> Result<String, JsValue> {
    gizza_ai_bitwise_calculator_core::compute(a, op, b, bits)
        .map_err(|e| JsValue::from_str(&e))
}
