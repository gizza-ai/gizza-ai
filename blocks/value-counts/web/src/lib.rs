//! Browser-facing wasm-bindgen wrapper for /tools/value-counts/.
//! Field order MUST match meta.toml: data, column, delimiter, sort,
//! case_sensitive, include_empty.
use wasm_bindgen::prelude::*;

fn truthy_default_true(s: &str) -> bool {
    !matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "false" | "0" | "off" | "no"
    )
}

fn truthy_default_false(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

#[wasm_bindgen]
pub fn run(
    data: &str,
    column: &str,
    delimiter: &str,
    sort: &str,
    case_sensitive: &str,
    include_empty: &str,
) -> Result<String, JsValue> {
    let delim = if delimiter.trim().is_empty() {
        ","
    } else {
        delimiter
    };
    let order = if sort.trim().is_empty() {
        "count"
    } else {
        sort
    };
    gizza_ai_value_counts_core::value_counts(
        data,
        column,
        delim,
        order,
        truthy_default_true(case_sensitive),
        truthy_default_false(include_empty),
    )
    .map_err(|e| JsValue::from_str(&e))
}
