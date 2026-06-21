//! Browser-facing wasm-bindgen wrapper for /tools/descriptive-stats/.
use gizza_ai_descriptive_stats_core::summary;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(numbers: &str) -> Result<String, JsValue> {
    summary(numbers).map_err(|e| JsValue::from_str(&e))
}
