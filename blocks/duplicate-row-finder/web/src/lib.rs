//! Browser-facing wasm-bindgen wrapper for /tools/duplicate-row-finder/.
//! tool.js passes every field value as a raw string (meta.toml input order), so
//! this parses the boolean/enum fields here and delegates to the core.
use gizza_ai_duplicate_row_finder_core::find;
use wasm_bindgen::prelude::*;

fn truthy(v: &str) -> bool {
    matches!(v.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
pub fn run(
    data: &str,
    columns: &str,
    header: &str,
    delimiter: &str,
    ignore_case: &str,
    ignore_whitespace: &str,
    output: &str,
) -> Result<String, JsValue> {
    let delim = if delimiter.is_empty() { "," } else { delimiter };
    let out = if output.trim().is_empty() { "report" } else { output };
    find(
        data,
        columns,
        truthy(header),
        delim,
        truthy(ignore_case),
        truthy(ignore_whitespace),
        out,
    )
    .map_err(|e| JsValue::from_str(&e))
}
