//! Browser-facing wasm-bindgen wrapper for /tools/decimal-to-fraction/.
use gizza_ai_decimal_to_fraction_core::{convert_json, Inputs};
use wasm_bindgen::prelude::*;

fn opt_num(value: &str, field: &str) -> Result<Option<f64>, String> {
    let t = value.trim();
    if t.is_empty() {
        return Ok(None);
    }
    t.parse::<f64>()
        .map(Some)
        .map_err(|_| format!("{field} must be a number, got '{value}'"))
}

fn checkbox(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    )
}

#[wasm_bindgen]
pub fn run(
    decimal: &str,
    repeating_digits: &str,
    tolerance: &str,
    max_denominator: &str,
    denominator: &str,
    rounding: &str,
    reduce: &str,
) -> Result<String, JsValue> {
    let inputs = Inputs {
        decimal: decimal.to_string(),
        repeating_digits: opt_num(repeating_digits, "repeating_digits")
            .map_err(|e| JsValue::from_str(&e))?,
        tolerance: opt_num(tolerance, "tolerance").map_err(|e| JsValue::from_str(&e))?,
        max_denominator: opt_num(max_denominator, "max_denominator")
            .map_err(|e| JsValue::from_str(&e))?,
        denominator: opt_num(denominator, "denominator").map_err(|e| JsValue::from_str(&e))?,
        rounding: if rounding.trim().is_empty() {
            "nearest".to_string()
        } else {
            rounding.to_string()
        },
        reduce: Some(if reduce.trim().is_empty() {
            true
        } else {
            checkbox(reduce)
        }),
    };
    convert_json(&inputs).map_err(|e| JsValue::from_str(&e))
}
