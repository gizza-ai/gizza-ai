//! Browser-facing wasm-bindgen wrapper for /tools/interpolation/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    data: &str,
    method: &str,
    at: &str,
    boundary: &str,
    start_slope: &str,
    end_slope: &str,
    extrapolate: &str,
    resample: &str,
    derivative: &str,
    decimals: &str,
    coefficients: &str,
    output: &str,
) -> Result<String, JsValue> {
    let opts = gizza_ai_interpolation_core::Options {
        method: default_str(method, "linear").to_string(),
        at: at.to_string(),
        boundary: default_str(boundary, "natural").to_string(),
        start_slope: parse_f64_default(start_slope, 0.0, "start_slope")?,
        end_slope: parse_f64_default(end_slope, 0.0, "end_slope")?,
        extrapolate: default_str(extrapolate, "error").to_string(),
        resample: parse_usize_default(resample, 0, "resample")?,
        derivative: parse_usize_default(derivative, 0, "derivative")?,
        decimals: parse_usize_default(decimals, 6, "decimals")?,
        coefficients: truthy(coefficients, false),
        output: default_str(output, "values").to_string(),
    };
    gizza_ai_interpolation_core::interpolate(data, &opts).map_err(|e| JsValue::from_str(&e))
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
            .map_err(|_| JsValue::from_str(&format!("{name} must be a whole number")))
    }
}
