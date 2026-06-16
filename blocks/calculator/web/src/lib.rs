//! Browser-facing wasm-bindgen wrapper around `gizza-ai-calculator-core`.
//! Compiled with wasm-pack for the standalone /tools/calculator/ page.

use wasm_bindgen::prelude::*;

/// Evaluate an arithmetic expression. Throws a JS error string on failure.
#[wasm_bindgen]
pub fn evaluate(expr: &str) -> Result<f64, JsValue> {
    gizza_ai_calculator_core::evaluate(expr).map_err(|e| JsValue::from_str(&e))
}
