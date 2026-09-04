//! Browser-facing wasm-bindgen wrapper for /tools/confusion-matrix/.
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

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    actual: &str,
    predicted: &str,
    labels: &str,
    positive_label: &str,
    input_format: &str,
    separator: &str,
    header: &str,
    normalize: &str,
    beta: &str,
    decimals: &str,
    percent: &str,
    format: &str,
) -> Result<String, JsValue> {
    gizza_ai_confusion_matrix_core::run(
        actual,
        predicted,
        labels,
        positive_label,
        if input_format.trim().is_empty() {
            "auto"
        } else {
            input_format
        },
        if separator.trim().is_empty() {
            "auto"
        } else {
            separator
        },
        if header.trim().is_empty() {
            "auto"
        } else {
            header
        },
        if normalize.trim().is_empty() {
            "none"
        } else {
            normalize
        },
        parse_f64_default(beta, 1.0, "beta")?,
        parse_f64_default(decimals, 4.0, "decimals")?,
        truthy(percent, false),
        if format.trim().is_empty() {
            "markdown"
        } else {
            format
        },
    )
    .map_err(|e| JsValue::from_str(&e))
}
