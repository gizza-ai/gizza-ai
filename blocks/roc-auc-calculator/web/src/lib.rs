//! Browser-facing wasm-bindgen wrapper for /tools/roc-auc-calculator/.
//! Field order MUST match meta.toml: data, labels, input_format, column_order,
//! separator, header, positive_label, optimize, cost_ratio, threshold,
//! confidence_level, table_rows, plot, decimals, percent, format.
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
    data: &str,
    labels: &str,
    input_format: &str,
    column_order: &str,
    separator: &str,
    header: &str,
    positive_label: &str,
    optimize: &str,
    cost_ratio: &str,
    threshold: &str,
    confidence_level: &str,
    table_rows: &str,
    plot: &str,
    decimals: &str,
    percent: &str,
    format: &str,
) -> Result<String, JsValue> {
    let cost_ratio = number(
        cost_ratio,
        1.0,
        "cost ratio must be a positive number, for example 1 or 10",
    )?;
    let table_rows = number(
        table_rows,
        12.0,
        "threshold table rows must be a whole number between 0 and 200",
    )?;
    let decimals = number(
        decimals,
        4.0,
        "decimals must be a whole number between 0 and 10",
    )?;
    gizza_ai_roc_auc_calculator_core::run(
        data,
        labels,
        input_format,
        column_order,
        separator,
        header,
        positive_label,
        optimize,
        cost_ratio,
        threshold,
        confidence_level,
        table_rows,
        flag(plot, true),
        decimals,
        flag(percent, false),
        format,
    )
    .map_err(|e| JsValue::from_str(&e))
}
