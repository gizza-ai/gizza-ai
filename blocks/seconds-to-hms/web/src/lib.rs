//! Browser-facing wasm-bindgen wrapper for /tools/seconds-to-hms/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(seconds: &str, format: &str, decimals: &str) -> Result<String, JsValue> {
    let seconds = seconds
        .trim()
        .parse::<f64>()
        .map_err(|_| JsValue::from_str("seconds must be a finite number"))?;
    let decimals = decimals.trim().parse::<u32>().unwrap_or(0);
    gizza_ai_seconds_to_hms_core::to_hms(seconds, format.trim(), decimals)
        .map_err(|e| JsValue::from_str(&e))
}
