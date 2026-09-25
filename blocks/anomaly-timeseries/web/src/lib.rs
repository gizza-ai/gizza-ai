//! Browser-facing wasm-bindgen wrapper for /tools/anomaly-timeseries/.
//! Compiled with wasm-pack for the standalone page. The page hands EVERY field
//! over as a string, so each one is parsed here (blank = the documented default)
//! and the core owns all validation.
use gizza_ai_anomaly_timeseries_core::{render, Options};
use wasm_bindgen::prelude::*;

fn boolish(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

fn text_or(s: &str, fallback: &str) -> String {
    if s.trim().is_empty() {
        fallback.into()
    } else {
        s.trim().into()
    }
}

fn parse_u32(name: &str, s: &str, fallback: u32) -> Result<u32, JsValue> {
    let t = s.trim();
    if t.is_empty() {
        return Ok(fallback);
    }
    t.parse::<u32>().map_err(|_| {
        JsValue::from_str(&format!(
            "{name} must be a whole number 0 or more (got {t:?})"
        ))
    })
}

fn parse_f64(name: &str, s: &str, fallback: f64) -> Result<f64, JsValue> {
    let t = s.trim();
    if t.is_empty() {
        return Ok(fallback);
    }
    t.parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
        .ok_or_else(|| JsValue::from_str(&format!("{name} must be a number (got {t:?})")))
}

/// Score `series` and return the report as JSON, an aligned table, or CSV.
/// Throws a JS error string describing what was expected on invalid input.
#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    series: &str,
    method: &str,
    window: &str,
    min_periods: &str,
    threshold: &str,
    warn_threshold: &str,
    period: &str,
    tolerance: &str,
    direction: &str,
    center: &str,
    only_anomalies: &str,
    decimals: &str,
    output: &str,
) -> Result<String, JsValue> {
    let d = Options::default();
    let opts = Options {
        method: text_or(method, &d.method),
        window: parse_u32("window", window, d.window)?,
        min_periods: parse_u32("min_periods", min_periods, d.min_periods)?,
        period: parse_u32("period", period, d.period)?,
        threshold: parse_f64("threshold", threshold, d.threshold)?,
        warn_threshold: parse_f64("warn_threshold", warn_threshold, d.warn_threshold)?,
        tolerance: parse_f64("tolerance", tolerance, d.tolerance)?,
        direction: text_or(direction, &d.direction),
        center: boolish(center),
        only_anomalies: boolish(only_anomalies),
        decimals: parse_u32("decimals", decimals, d.decimals)?,
        output: text_or(output, &d.output),
    };
    render(series, &opts).map_err(|e| JsValue::from_str(&e))
}
