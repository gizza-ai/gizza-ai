//! Browser-facing wasm-bindgen wrapper for /tools/timesheet-calculator/.
//! Field order MUST match meta.toml: log, rate, rates, currency, round.
use wasm_bindgen::prelude::*;

/// Total the work log and return a pretty-printed JSON report. On a parse error
/// it throws the error string. `rate` arrives as an f64 field; `round` is the
/// billing increment in minutes as a string ("0", "6", …).
#[wasm_bindgen]
pub fn run(log: &str, rate: f64, rates: &str, currency: &str, round: &str) -> Result<String, JsValue> {
    let round_min: i64 = match round.trim() {
        "" => 0,
        v => v.parse().unwrap_or(0),
    };
    gizza_ai_timesheet_calculator_core::compute_json(log, rate, rates, currency, round_min)
        .map_err(|e| JsValue::from_str(&e))
}
