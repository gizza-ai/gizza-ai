//! Browser-facing wasm-bindgen wrapper for /tools/column-value-diff/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    left: &str,
    right: &str,
    key: &str,
    value: &str,
    delimiter: &str,
    header: &str,
    ignore_case: &str,
    ignore_whitespace: &str,
    include_unmatched: &str,
    format: &str,
) -> Result<String, JsValue> {
    // header defaults ON (checked); the other flags default OFF.
    let header = truthy_default(header, true);
    let ignore_case = truthy_default(ignore_case, false);
    let ignore_whitespace = truthy_default(ignore_whitespace, false);
    let include_unmatched = truthy_default(include_unmatched, false);
    gizza_ai_column_value_diff_core::run(
        left,
        right,
        key,
        value,
        delimiter,
        header,
        ignore_case,
        ignore_whitespace,
        include_unmatched,
        format,
    )
    .map_err(|e| JsValue::from_str(&e))
}

/// Parse a checkbox/flag value; empty falls back to `default`.
fn truthy_default(s: &str, default: bool) -> bool {
    match s.trim().to_ascii_lowercase().as_str() {
        "" => default,
        "true" | "1" | "yes" | "on" => true,
        _ => false,
    }
}
