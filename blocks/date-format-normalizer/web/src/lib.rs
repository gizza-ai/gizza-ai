//! Browser-facing wasm-bindgen wrapper for /tools/date-format-normalizer/.
//! The page driver hands every field through as a raw string, so each param is
//! `&str` and parsed here; the core owns all validation and clamping.
use wasm_bindgen::prelude::*;

/// Positive-truthy parse for a checkbox field (`"true"`/`"false"` from the page).
fn flag(v: &str, default: bool) -> bool {
    match v.trim() {
        "" => default,
        s => matches!(s, "true" | "1" | "on" | "yes"),
    }
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    text: &str,
    output_format: &str,
    custom_format: &str,
    separator: &str,
    month_style: &str,
    year_style: &str,
    leading_zeros: &str,
    input_order: &str,
    two_digit_year_pivot: &str,
    keep_time: &str,
    time_style: &str,
    output_timezone: &str,
    detect_timestamps: &str,
    output_mode: &str,
) -> Result<String, JsValue> {
    let pivot: i64 = match two_digit_year_pivot.trim() {
        "" => 68,
        s => s.parse::<f64>().map(|v| v.round() as i64).map_err(|_| {
            JsValue::from_str(&format!(
                "two_digit_year_pivot must be a whole number between 0 and 99 — got '{s}'"
            ))
        })?,
    };
    gizza_ai_date_format_normalizer_core::run(
        text,
        output_format,
        custom_format,
        separator,
        month_style,
        year_style,
        flag(leading_zeros, true),
        input_order,
        pivot,
        flag(keep_time, true),
        time_style,
        output_timezone,
        flag(detect_timestamps, false),
        output_mode,
    )
    .map_err(|e| JsValue::from_str(&e))
}
