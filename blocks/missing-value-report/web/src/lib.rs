//! Browser-facing wasm-bindgen wrapper for /tools/missing-value-report/.
//! Field order MUST match meta.toml: input, delimiter, na_values, sort,
//! include_patterns, max_patterns. Fields arrive as strings (the checkbox
//! sends "true"/"false"). The core owns all validation and error messages.
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

fn parse_i64(s: &str, default: i64) -> i64 {
    let t = s.trim();
    if t.is_empty() {
        default
    } else {
        t.parse().unwrap_or(default)
    }
}

/// Profile a CSV/table for missing values (per-column counts + pattern grid).
///
/// - `input`: the CSV/delimited table text (header row required).
/// - `delimiter`: single char or 'comma'/'tab'/'semicolon'/'pipe' (empty → comma).
/// - `na_values`: comma-separated extra missing tokens (empty → blanks only).
/// - `sort`: `missing` (default) | `column` | `name`.
/// - `include_patterns`: checkbox `"true"`/`"false"` (empty → false).
/// - `max_patterns`: integer cap on grid rows (empty → 10).
///
/// Throws a JS error string on invalid input or an out-of-range option.
#[wasm_bindgen]
pub fn run(
    input: &str,
    delimiter: &str,
    na_values: &str,
    sort: &str,
    include_patterns: &str,
    max_patterns: &str,
) -> Result<String, JsValue> {
    gizza_ai_missing_value_report_core::run(
        input,
        delimiter,
        na_values,
        sort,
        truthy(include_patterns),
        parse_i64(max_patterns, 10),
    )
    .map_err(|e| JsValue::from_str(&e))
}
