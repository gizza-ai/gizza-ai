//! Browser-facing wasm-bindgen wrapper for /tools/csv-cell-diff/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    left: &str,
    right: &str,
    key: &str,
    delimiter: &str,
    header: &str,
    ignore_case: &str,
    ignore_whitespace: &str,
    format: &str,
) -> Result<String, JsValue> {
    // header defaults ON (checked); the other flags default OFF.
    let header = truthy_default(header, true);
    let ignore_case = truthy_default(ignore_case, false);
    let ignore_whitespace = truthy_default(ignore_whitespace, false);
    gizza_ai_csv_cell_diff_core::run(
        left,
        right,
        key,
        delimiter,
        header,
        ignore_case,
        ignore_whitespace,
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
