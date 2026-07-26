//! Browser-facing wasm-bindgen wrapper for /tools/csv-collapse-rows/.
//!
//! The standalone page passes every field value as a string (in the meta.toml
//! `[[input]]` order), so the boolean `dedupe`/`skip_empty`/`has_header` params
//! arrive as strings and are parsed here. `dedupe` defaults OFF; `skip_empty`
//! and `has_header` default ON (blank → true).
use gizza_ai_csv_collapse_rows_core::collapse_rows;
use wasm_bindgen::prelude::*;

/// Positive-truthy: an explicit true-ish string.
fn truthy(v: &str) -> bool {
    matches!(v.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

/// Default-ON boolean: blank stays true; only an explicit falsey turns it off.
fn truthy_default_on(v: &str) -> bool {
    !matches!(v.trim().to_ascii_lowercase().as_str(), "false" | "0" | "off" | "no")
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    data: &str,
    key_columns: &str,
    collapse_column: &str,
    separator: &str,
    dedupe: &str,
    skip_empty: &str,
    sort_values: &str,
    delimiter: &str,
    has_header: &str,
) -> Result<String, JsValue> {
    let sep = if separator.is_empty() { ", " } else { separator };
    let sort = if sort_values.is_empty() { "none" } else { sort_values };
    let delim = if delimiter.is_empty() { "comma" } else { delimiter };
    collapse_rows(
        data,
        key_columns,
        collapse_column,
        sep,
        truthy(dedupe),
        truthy_default_on(skip_empty),
        sort,
        delim,
        truthy_default_on(has_header),
    )
    .map_err(|e| JsValue::from_str(&e))
}
