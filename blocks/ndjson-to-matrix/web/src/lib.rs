//! Browser-facing wasm-bindgen wrapper for /tools/ndjson-to-matrix/.
use wasm_bindgen::prelude::*;

fn parse_i64(value: &str, default: i64) -> i64 {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        default
    } else {
        trimmed.parse().unwrap_or(default)
    }
}

fn truthy(value: &str) -> bool {
    matches!(value.trim(), "true" | "1" | "on" | "yes")
}

fn or_default<'a>(value: &'a str, default: &'a str) -> &'a str {
    if value.trim().is_empty() {
        default
    } else {
        value
    }
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    data: &str,
    format: &str,
    delimiter: &str,
    separator: &str,
    arrays: &str,
    columns: &str,
    column_order: &str,
    fill: &str,
    headers: &str,
    row_index: &str,
    numeric_only: &str,
    transpose: &str,
    max_depth: &str,
    limit: &str,
    invalid: &str,
) -> Result<String, JsValue> {
    gizza_ai_ndjson_to_matrix_core::run(
        data,
        or_default(format, "csv"),
        or_default(delimiter, "comma"),
        // A cleared page field means "use the default", not "no separator".
        if separator.is_empty() { "." } else { separator },
        or_default(arrays, "index"),
        columns,
        or_default(column_order, "first-seen"),
        fill,
        truthy(headers),
        truthy(row_index),
        truthy(numeric_only),
        truthy(transpose),
        parse_i64(max_depth, 0),
        parse_i64(limit, 0),
        or_default(invalid, "error"),
    )
    .map_err(|e| JsValue::from_str(&e))
}
