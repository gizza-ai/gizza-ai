//! Browser-facing wasm-bindgen wrapper for /tools/bollinger-bands/.
use gizza_ai_bollinger_bands_core::summary;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(prices: &str, period: &str, num_std: &str) -> Result<String, JsValue> {
    let period: usize = period
        .trim()
        .parse()
        .map_err(|_| JsValue::from_str("period must be a positive whole number"))?;
    let num_std: f64 = num_std
        .trim()
        .parse()
        .map_err(|_| JsValue::from_str("num_std must be a number"))?;
    summary(prices, period, num_std).map_err(|e| JsValue::from_str(&e))
}
