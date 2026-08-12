//! Browser-facing wasm-bindgen wrapper for /tools/exponential-smoother/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    series: &str,
    mode: &str,
    alpha: &str,
    span: &str,
    halflife: &str,
    com: &str,
    adjust: &str,
    ignore_na: &str,
    min_periods: &str,
    forecast: &str,
    output: &str,
) -> Result<String, JsValue> {
    let opts = gizza_ai_exponential_smoother_core::Options {
        mode: default_str(mode, "alpha").to_string(),
        alpha: parse_f64_default(alpha, 0.3, "alpha")?,
        span: parse_f64_default(span, 5.0, "span")?,
        halflife: parse_f64_default(halflife, 3.0, "halflife")?,
        com: parse_f64_default(com, 2.0, "com")?,
        adjust: truthy(adjust, true),
        ignore_na: truthy(ignore_na, false),
        min_periods: parse_usize_default(min_periods, 0, "min_periods")?,
        forecast: parse_usize_default(forecast, 0, "forecast")?,
        output: default_str(output, "json").to_string(),
    };
    gizza_ai_exponential_smoother_core::smooth(series, &opts).map_err(|e| JsValue::from_str(&e))
}

fn default_str<'a>(v: &'a str, default: &'a str) -> &'a str {
    if v.trim().is_empty() {
        default
    } else {
        v.trim()
    }
}

/// Checkboxes arrive as "true"/"false"; match positive-truthy only so an
/// unchecked box never reads as on.
fn truthy(v: &str, default: bool) -> bool {
    let s = v.trim().to_ascii_lowercase();
    if s.is_empty() {
        default
    } else {
        matches!(s.as_str(), "true" | "1" | "on" | "yes")
    }
}

fn parse_f64_default(v: &str, default: f64, name: &str) -> Result<f64, JsValue> {
    if v.trim().is_empty() {
        Ok(default)
    } else {
        v.trim()
            .parse::<f64>()
            .map_err(|_| JsValue::from_str(&format!("{name} must be a number")))
    }
}

fn parse_usize_default(v: &str, default: usize, name: &str) -> Result<usize, JsValue> {
    if v.trim().is_empty() {
        Ok(default)
    } else {
        v.trim()
            .parse::<usize>()
            .map_err(|_| JsValue::from_str(&format!("{name} must be a whole number")))
    }
}
