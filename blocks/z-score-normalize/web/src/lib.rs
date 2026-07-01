//! Browser-facing wasm-bindgen wrapper for /tools/z-score-normalize/.
use gizza_ai_z_score_normalize_core::summary;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(numbers: &str, method: &str, sample: &str) -> Result<String, JsValue> {
    let use_sample = matches!(sample, "true" | "1" | "on" | "yes");
    let method = if method.is_empty() { "z-score" } else { method };
    summary(numbers, method, use_sample).map_err(|e| JsValue::from_str(&e))
}
