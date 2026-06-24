//! Browser-facing wasm-bindgen wrapper for /tools/outlier-detector/.
use gizza_ai_outlier_detector_core::summary;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(numbers: &str, z_threshold: &str, iqr_k: &str) -> Result<String, JsValue> {
    let z = z_threshold
        .trim()
        .parse::<f64>()
        .map_err(|_| JsValue::from_str("z_threshold must be a number"))?;
    let k = iqr_k
        .trim()
        .parse::<f64>()
        .map_err(|_| JsValue::from_str("iqr_k must be a number"))?;
    summary(numbers, z, k).map_err(|e| JsValue::from_str(&e))
}
