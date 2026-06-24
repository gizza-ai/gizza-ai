//! Browser-facing wasm-bindgen wrapper for /tools/column-math/.
//! Compiled with wasm-pack for the standalone /tools/column-math/ page.
use wasm_bindgen::prelude::*;

/// Element-wise arithmetic between two numeric columns.
///
/// The standalone tool page passes every field value as a string:
/// - `a`, `b`: the two columns (numbers separated by commas, spaces, or newlines).
/// - `operation`: `"add"` (blank → add) | `"subtract"` | `"multiply"` | `"divide"`.
///
/// Throws a JS error string on an invalid operation, a length mismatch, a
/// non-numeric value, or a division by zero.
#[wasm_bindgen]
pub fn run(a: &str, b: &str, operation: &str) -> Result<String, JsValue> {
    gizza_ai_column_math_core::compute(a, b, operation).map_err(|e| JsValue::from_str(&e))
}
