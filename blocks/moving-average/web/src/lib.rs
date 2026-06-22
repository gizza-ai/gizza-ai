//! Browser-facing wasm-bindgen wrapper for /tools/moving-average/.
//! Compiled with wasm-pack for the standalone /tools/moving-average/ page.
use wasm_bindgen::prelude::*;

/// Compute SMA + EMA over `series` with look-back window `period`.
///
/// The standalone tool page passes every field value as a string, so `period`
/// arrives as a string and is parsed here (blank/unparseable → 3, the default).
/// Returns pretty-printed JSON `{count, period, smoothing_factor, sma[], ema[]}`.
/// Throws a JS error string on invalid/insufficient input.
#[wasm_bindgen]
pub fn run(series: &str, period: &str) -> Result<String, JsValue> {
    let period = period.trim().parse::<u32>().unwrap_or(3);
    let ma = gizza_ai_moving_average_core::compute(series, period)
        .map_err(|e| JsValue::from_str(&e))?;
    serde_json::to_string_pretty(&ma).map_err(|e| JsValue::from_str(&e.to_string()))
}
