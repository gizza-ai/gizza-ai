//! Browser-facing wasm-bindgen wrapper for /tools/chi-square-test/.
//! Field order MUST match meta.toml: mode, observed, expected, yates.
use gizza_ai_chi_square_test_core::summary;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(mode: &str, observed: &str, expected: &str, yates: &str) -> Result<String, JsValue> {
    summary(observed, expected, mode, yates).map_err(|e| JsValue::from_str(&e))
}
