//! Browser-facing wasm-bindgen wrapper for /tools/period-predictor/.
//! Compiled with wasm-pack for the standalone /tools/period-predictor/ page.
//!
//! Field order MUST match meta.toml: last_period, cycle_length, period_length,
//! luteal_phase, cycles. The numeric fields arrive as strings from the form;
//! blank ones fall back to the schema defaults (28 / 5 / 14 / 6).
use wasm_bindgen::prelude::*;

/// Parse a numeric form field, falling back to `default` when it is blank.
fn parse_int(s: &str, field: &str, default: i64) -> Result<i64, JsValue> {
    let t = s.trim();
    if t.is_empty() {
        return Ok(default);
    }
    t.parse::<i64>()
        .map_err(|_| JsValue::from_str(&format!("{field} must be a whole number")))
}

/// Predict upcoming periods and return a pretty-printed JSON object. On a parse
/// or validation error it throws the error string.
#[wasm_bindgen]
pub fn run(
    last_period: &str,
    cycle_length: &str,
    period_length: &str,
    luteal_phase: &str,
    cycles: &str,
) -> Result<String, JsValue> {
    let cycle_length = parse_int(cycle_length, "cycle_length", 28)?;
    let period_length = parse_int(period_length, "period_length", 5)?;
    let luteal_phase = parse_int(luteal_phase, "luteal_phase", 14)?;
    let cycles = parse_int(cycles, "cycles", 6)?;
    gizza_ai_period_predictor_core::period_predict_json(
        last_period,
        cycle_length,
        period_length,
        luteal_phase,
        cycles,
    )
    .map_err(|e| JsValue::from_str(&e))
}
