//! Browser-facing wasm-bindgen wrapper for /tools/rsi-calculator/.
//! Compiled with wasm-pack for the standalone /tools/rsi-calculator/ page.
use wasm_bindgen::prelude::*;

/// Compute the RSI over `prices`.
///
/// The standalone tool page passes every field value as a string, so the period
/// and thresholds arrive as strings and are parsed here (blank/unparseable → the
/// standard defaults 14 / 70 / 30). Returns pretty-printed JSON. Throws a JS
/// error string on invalid/insufficient input.
#[wasm_bindgen]
pub fn run(prices: &str, period: &str, overbought: &str, oversold: &str) -> Result<String, JsValue> {
    let period = period.trim().parse::<u32>().unwrap_or(14);
    let overbought = overbought.trim().parse::<f64>().unwrap_or(70.0);
    let oversold = oversold.trim().parse::<f64>().unwrap_or(30.0);
    let r = gizza_ai_rsi_calculator_core::compute(prices, period, overbought, oversold)
        .map_err(|e| JsValue::from_str(&e))?;
    serde_json::to_string_pretty(&r).map_err(|e| JsValue::from_str(&e.to_string()))
}
