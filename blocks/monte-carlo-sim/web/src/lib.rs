//! Browser-facing wasm-bindgen wrapper for /tools/monte-carlo-sim/.
//! Field order MUST match meta.toml: variables, model, trials, seed, threshold,
//! threshold_direction, histogram_bins. Fields arrive as strings; empty
//! optional fields fall back to the schema defaults.
use gizza_ai_monte_carlo_sim_core::report;
use wasm_bindgen::prelude::*;

fn parse_i64(s: &str, field: &str, default: i64) -> Result<i64, JsValue> {
    let t = s.trim();
    if t.is_empty() {
        return Ok(default);
    }
    t.parse::<i64>()
        .map_err(|_| JsValue::from_str(&format!("{field} must be a whole number")))
}

fn parse_u64(s: &str, field: &str, default: u64) -> Result<u64, JsValue> {
    let t = s.trim();
    if t.is_empty() {
        return Ok(default);
    }
    t.parse::<u64>()
        .map_err(|_| JsValue::from_str(&format!("{field} must be a non-negative whole number")))
}

#[wasm_bindgen]
pub fn run(
    variables: &str,
    model: &str,
    trials: &str,
    seed: &str,
    threshold: &str,
    threshold_direction: &str,
    histogram_bins: &str,
) -> Result<String, JsValue> {
    let trials = parse_i64(trials, "trials", 10_000)?;
    let seed = parse_u64(seed, "seed", 42)?;
    let threshold = {
        let t = threshold.trim();
        if t.is_empty() {
            None
        } else {
            Some(
                t.parse::<f64>()
                    .map_err(|_| JsValue::from_str("threshold must be a number"))?,
            )
        }
    };
    // An empty direction field falls back to the schema default.
    let dir = if threshold_direction.trim().is_empty() {
        "at-or-above"
    } else {
        threshold_direction
    };
    let bins = parse_i64(histogram_bins, "histogram_bins", 20)?;
    report(variables, model, trials, seed, threshold, dir, bins).map_err(|e| JsValue::from_str(&e))
}
