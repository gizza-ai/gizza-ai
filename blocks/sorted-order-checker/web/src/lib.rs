//! Browser-facing wasm-bindgen wrapper for /tools/sorted-order-checker/.
//! The page passes every field as a string, in meta.toml order.
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

#[wasm_bindgen]
pub fn run(
    numbers: &str,
    order: &str,
    strict: &str,
    separator: &str,
    strip_thousands: &str,
    non_numeric: &str,
    max_issues: &str,
    format: &str,
) -> Result<String, JsValue> {
    let max_issues = match max_issues.trim() {
        "" => 20,
        raw => raw
            .parse::<f64>()
            .ok()
            .filter(|v| v.is_finite() && *v >= 0.0)
            .map(|v| v as u64)
            .ok_or_else(|| JsValue::from_str("max_issues must be a whole number between 1 and 1000"))?,
    };
    gizza_ai_sorted_order_checker_core::run_with_options(
        numbers,
        order,
        truthy(strict),
        separator,
        truthy(strip_thousands),
        non_numeric,
        max_issues,
        format,
    )
    .map_err(|e| JsValue::from_str(&e))
}
