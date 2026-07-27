//! Browser-facing wasm-bindgen wrapper for /tools/target-mean-encoder/.
use wasm_bindgen::prelude::*;

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

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    data: &str,
    category: &str,
    target: &str,
    smoothing: &str,
    leave_one_out: &str,
    output: &str,
    unknown: &str,
    decimals: &str,
    has_header: &str,
    delimiter: &str,
) -> Result<String, JsValue> {
    gizza_ai_target_mean_encoder_core::encode(
        data,
        category,
        target,
        parse_f64_default(smoothing, 0.0, "smoothing")?,
        truthy(leave_one_out, false),
        if output.trim().is_empty() {
            "replace"
        } else {
            output
        },
        if unknown.trim().is_empty() {
            "global-mean"
        } else {
            unknown
        },
        parse_usize_default(decimals, 4, "decimals")?,
        truthy(has_header, true),
        if delimiter.trim().is_empty() {
            "comma"
        } else {
            delimiter
        },
    )
    .map_err(|e| JsValue::from_str(&e))
}
