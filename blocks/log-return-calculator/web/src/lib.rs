//! Browser-facing wasm-bindgen wrapper for /tools/log-return-calculator/.
//! Every field arrives as a string from the page driver, so the numeric and
//! boolean params are parsed here and the core keeps its typed signature.
use wasm_bindgen::prelude::*;

fn parse_f64(s: &str, default: f64) -> f64 {
    let t = s.trim();
    if t.is_empty() {
        default
    } else {
        t.parse().unwrap_or(default)
    }
}

fn parse_i64(s: &str, default: i64) -> i64 {
    let t = s.trim();
    if t.is_empty() {
        default
    } else {
        t.parse().unwrap_or(default)
    }
}

fn truthy(s: &str) -> bool {
    matches!(s.trim(), "true" | "1" | "on" | "yes")
}

fn or_default<'a>(s: &'a str, default: &'a str) -> &'a str {
    if s.trim().is_empty() {
        default
    } else {
        s
    }
}

#[wasm_bindgen]
pub fn run(
    prices: &str,
    has_header: &str,
    periods_per_year: &str,
    unit: &str,
    decimals: &str,
    output: &str,
) -> Result<String, JsValue> {
    gizza_ai_log_return_calculator_core::summary(
        prices,
        truthy(has_header),
        parse_f64(periods_per_year, 252.0),
        or_default(unit, "percent"),
        parse_i64(decimals, 4),
        or_default(output, "summary"),
    )
    .map_err(|e| JsValue::from_str(&e))
}
