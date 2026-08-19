//! Browser-facing wasm-bindgen wrapper for /tools/spline-smoother/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    input: &str,
    mode: &str,
    smoothing: &str,
    lambda: &str,
    df: &str,
    criterion: &str,
    weights: &str,
    predict_at: &str,
    resample: &str,
    coefficients: &str,
    output: &str,
) -> Result<String, JsValue> {
    let opts = gizza_ai_spline_smoother_core::Options {
        mode: default_str(mode, "auto").to_string(),
        smoothing: parse_f64_default(smoothing, 0.99, "smoothing")?,
        lambda: parse_f64_default(lambda, 1.0, "lambda")?,
        df: parse_f64_default(df, 5.0, "df")?,
        criterion: default_str(criterion, "gcv").to_string(),
        weights: weights.to_string(),
        predict_at: predict_at.to_string(),
        resample: parse_usize_default(resample, 0, "resample")?,
        coefficients: truthy(coefficients, false),
        output: default_str(output, "json").to_string(),
    };
    gizza_ai_spline_smoother_core::smooth(input, &opts).map_err(|e| JsValue::from_str(&e))
}

fn default_str<'a>(v: &'a str, default: &'a str) -> &'a str {
    if v.trim().is_empty() {
        default
    } else {
        v.trim()
    }
}

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
            .map_err(|_| JsValue::from_str(&format!("{name} must be an integer")))
    }
}
