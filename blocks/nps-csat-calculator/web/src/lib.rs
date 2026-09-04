//! Browser-facing wasm-bindgen wrapper for /tools/nps-csat-calculator/.
//! Field order MUST match page/meta.toml: ratings, metric, input, scale,
//! threshold, confidence, decimals, distribution, format. Each value arrives as
//! a string (checkboxes send "true"/"false"); blank numeric fields fall back to
//! the documented defaults.
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

/// Parse an optional whole-number field: blank → `fallback`.
fn parse_int(s: &str, field: &str, fallback: i64) -> Result<i64, JsValue> {
    let t = s.trim();
    if t.is_empty() {
        return Ok(fallback);
    }
    t.parse::<i64>()
        .map_err(|_| JsValue::from_str(&format!("{field} must be a whole number (got '{t}')")))
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    ratings: &str,
    metric: &str,
    input: &str,
    scale: &str,
    threshold: &str,
    confidence: &str,
    decimals: &str,
    distribution: &str,
    format: &str,
) -> Result<String, JsValue> {
    let threshold = parse_int(threshold, "threshold", -1)?;
    let decimals = parse_int(decimals, "decimals", 1)?;
    gizza_ai_nps_csat_calculator_core::calculate(
        ratings,
        metric,
        input,
        scale,
        threshold,
        confidence,
        decimals,
        truthy(distribution),
        format,
    )
    .map_err(|e| JsValue::from_str(&e))
}
