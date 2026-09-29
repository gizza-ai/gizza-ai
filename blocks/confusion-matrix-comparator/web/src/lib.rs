//! Browser-facing wasm-bindgen wrapper for /tools/confusion-matrix-comparator/.
//! Field order MUST match meta.toml: matrix_a, matrix_b, labels, name_a, name_b,
//! input_format, orientation, separator, header, beta, sort_by, significance,
//! confidence_level, matrix_delta, decimals, percent, format.
use wasm_bindgen::prelude::*;

/// A number field: empty keeps the default, anything else must parse.
fn number(value: &str, default: f64, msg: &str) -> Result<f64, JsValue> {
    let v = value.trim();
    if v.is_empty() {
        return Ok(default);
    }
    v.parse().map_err(|_| JsValue::from_str(msg))
}

/// A checkbox: the page sends "true"/"false"; an absent value in a deep link
/// keeps the field's default.
fn flag(value: &str, default: bool) -> bool {
    let v = value.trim();
    if v.is_empty() {
        return default;
    }
    matches!(v.to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    matrix_a: &str,
    matrix_b: &str,
    labels: &str,
    name_a: &str,
    name_b: &str,
    input_format: &str,
    orientation: &str,
    separator: &str,
    header: &str,
    beta: &str,
    sort_by: &str,
    significance: &str,
    confidence_level: &str,
    matrix_delta: &str,
    decimals: &str,
    percent: &str,
    format: &str,
) -> Result<String, JsValue> {
    let beta = number(
        beta,
        1.0,
        "the F-score weight must be a number between 0.1 and 10, for example 1 or 2",
    )?;
    let decimals = number(
        decimals,
        4.0,
        "decimals must be a whole number between 0 and 10",
    )?;
    gizza_ai_confusion_matrix_comparator_core::run(
        matrix_a,
        matrix_b,
        labels,
        name_a,
        name_b,
        input_format,
        orientation,
        separator,
        header,
        beta,
        sort_by,
        flag(significance, true),
        confidence_level,
        flag(matrix_delta, true),
        decimals,
        flag(percent, false),
        format,
    )
    .map_err(|e| JsValue::from_str(&e))
}
