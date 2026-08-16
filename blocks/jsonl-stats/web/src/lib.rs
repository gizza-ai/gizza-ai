//! Browser-facing wasm-bindgen wrapper for /tools/jsonl-stats/.
use wasm_bindgen::prelude::*;

fn parse_i64(s: &str, default: i64) -> i64 {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        default
    } else {
        trimmed.parse().unwrap_or(default)
    }
}

fn truthy(s: &str) -> bool {
    matches!(s.trim(), "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
pub fn run(
    input: &str,
    depth: &str,
    format: &str,
    sort: &str,
    max_keys: &str,
    samples: &str,
    value_stats: &str,
    distinct: &str,
    invalid: &str,
) -> Result<String, JsValue> {
    gizza_ai_jsonl_stats_core::run(
        input,
        parse_i64(depth, 1),
        if format.trim().is_empty() {
            "text"
        } else {
            format
        },
        if sort.trim().is_empty() {
            "frequency"
        } else {
            sort
        },
        parse_i64(max_keys, 0),
        parse_i64(samples, 2),
        truthy(value_stats),
        truthy(distinct),
        if invalid.trim().is_empty() {
            "report"
        } else {
            invalid
        },
    )
    .map_err(|e| JsValue::from_str(&e))
}
