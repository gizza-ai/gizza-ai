//! Browser-facing wasm-bindgen wrapper for /tools/returns-risk-analyzer/.
//! Field order MUST match meta.toml: returns, periods_per_year, risk_free_rate,
//! target_return, has_header. The page marshals every field as a string, so we
//! parse the numeric/boolean ones here.
use gizza_ai_returns_risk_analyzer_core::summary;
use wasm_bindgen::prelude::*;

fn parse_pct(v: &str, field: &str) -> Result<f64, String> {
    let t = v.trim().trim_end_matches('%').trim();
    if t.is_empty() {
        return Ok(0.0);
    }
    t.parse::<f64>()
        .map_err(|_| format!("{field} must be a number, got '{v}'"))
}

#[wasm_bindgen]
pub fn run(
    returns: &str,
    periods_per_year: &str,
    risk_free_rate: &str,
    target_return: &str,
    has_header: &str,
) -> Result<String, JsValue> {
    let ppy_str = periods_per_year.trim();
    let ppy: f64 = if ppy_str.is_empty() {
        252.0
    } else {
        ppy_str
            .parse()
            .map_err(|_| JsValue::from_str("periods per year must be a number"))?
    };
    let rf = parse_pct(risk_free_rate, "risk-free rate").map_err(|e| JsValue::from_str(&e))?;
    let target = parse_pct(target_return, "target return").map_err(|e| JsValue::from_str(&e))?;
    let header = matches!(has_header.trim().to_ascii_lowercase().as_str(), "true" | "1" | "yes" | "on");
    summary(returns, ppy, rf, target, header).map_err(|e| JsValue::from_str(&e))
}
