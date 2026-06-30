//! Browser-facing wasm-bindgen wrapper for /tools/macd-calculator/.
//! Compiled with wasm-pack for the standalone /tools/macd-calculator/ page.
use wasm_bindgen::prelude::*;

/// Compute the MACD line, signal line and histogram over `prices`.
///
/// The standalone tool page passes every field value as a string, so the
/// numeric periods arrive as strings and are parsed here (blank/unparseable →
/// the standard defaults 12 / 26 / 9). Returns pretty-printed JSON. Throws a JS
/// error string on invalid/insufficient input.
#[wasm_bindgen]
pub fn run(prices: &str, fast: &str, slow: &str, signal: &str) -> Result<String, JsValue> {
    let fast = fast.trim().parse::<u32>().unwrap_or(12);
    let slow = slow.trim().parse::<u32>().unwrap_or(26);
    let signal = signal.trim().parse::<u32>().unwrap_or(9);
    let m = gizza_ai_macd_calculator_core::compute(prices, fast, slow, signal)
        .map_err(|e| JsValue::from_str(&e))?;
    serde_json::to_string_pretty(&m).map_err(|e| JsValue::from_str(&e.to_string()))
}
