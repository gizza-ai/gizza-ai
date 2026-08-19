//! Browser-facing wasm-bindgen wrapper for /tools/drawdown-analyzer/.
//! Field order MUST match page/meta.toml: series, series_type, frequency,
//! start_date, has_header, top_n, recovery_cagr. The page marshals every field
//! as a string, so the numeric/boolean ones are parsed here; an empty field
//! falls back to the descriptor default.
use gizza_ai_drawdown_analyzer_core::run as analyze_summary;
use wasm_bindgen::prelude::*;

/// Parse an optional number field, tolerating a trailing `%` and blanks.
fn parse_num(v: &str, field: &str, fallback: f64) -> Result<f64, String> {
    let t = v.trim().trim_end_matches('%').trim();
    if t.is_empty() {
        return Ok(fallback);
    }
    t.parse::<f64>()
        .map_err(|_| format!("{field} must be a number, got '{v}'"))
}

#[wasm_bindgen]
pub fn run(
    series: &str,
    series_type: &str,
    frequency: &str,
    start_date: &str,
    has_header: &str,
    top_n: &str,
    recovery_cagr: &str,
) -> Result<String, JsValue> {
    let stype = match series_type.trim() {
        "" => "equity",
        s => s,
    };
    let freq = match frequency.trim() {
        "" => "period",
        f => f,
    };
    let header = matches!(
        has_header.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    );
    let n = parse_num(top_n, "episodes to list", 5.0).map_err(|e| JsValue::from_str(&e))?;
    if !n.is_finite() || n.fract() != 0.0 || !(1.0..=20.0).contains(&n) {
        return Err(JsValue::from_str(
            "episodes to list must be a whole number between 1 and 20",
        ));
    }
    let cagr = parse_num(recovery_cagr, "recovery rate", 0.0).map_err(|e| JsValue::from_str(&e))?;
    analyze_summary(series, stype, freq, start_date, header, n as usize, cagr)
        .map_err(|e| JsValue::from_str(&e))
}
