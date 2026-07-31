//! Browser-facing wasm-bindgen wrapper for /tools/numeric-range-check/.
use wasm_bindgen::prelude::*;

fn flag(s: &str, default: bool) -> bool {
    match s.trim().to_ascii_lowercase().as_str() {
        "" => default,
        "true" | "1" | "yes" | "on" => true,
        _ => false,
    }
}

/// Empty string → None (bound not set); otherwise parse a decimal.
fn parse_bound(s: &str) -> Option<f64> {
    let t = s.trim();
    if t.is_empty() {
        return None;
    }
    t.parse::<f64>().ok()
}

fn parse_max_issues(s: &str) -> usize {
    s.trim().parse::<usize>().unwrap_or(50)
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    data: &str,
    columns: &str,
    min: &str,
    max: &str,
    inclusive: &str,
    header: &str,
    delimiter: &str,
    non_numeric: &str,
    empty_ok: &str,
    max_issues: &str,
    format: &str,
) -> Result<String, JsValue> {
    gizza_ai_numeric_range_check_core::run(
        data,
        columns,
        parse_bound(min),
        parse_bound(max),
        flag(inclusive, true),
        flag(header, true),
        delimiter,
        non_numeric,
        flag(empty_ok, true),
        parse_max_issues(max_issues),
        format,
    )
    .map_err(|e| JsValue::from_str(&e))
}
