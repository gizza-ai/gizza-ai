//! Browser-facing wasm-bindgen wrapper for /tools/csv-cleaner/.
use gizza_ai_csv_cleaner_core::clean;
use wasm_bindgen::prelude::*;

fn truthy(v: &str) -> bool {
    matches!(v.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    data: &str,
    header: &str,
    delimiter: &str,
    output_delimiter: &str,
    trim: &str,
    dedupe: &str,
    drop_empty_rows: &str,
    empty_cells: &str,
    fill_value: &str,
    line_ending: &str,
) -> Result<String, JsValue> {
    let delim = if delimiter.is_empty() { "," } else { delimiter };
    let out_delim = if output_delimiter.is_empty() { "same" } else { output_delimiter };
    let empties = if empty_cells.is_empty() { "keep" } else { empty_cells };
    let le = if line_ending.is_empty() { "lf" } else { line_ending };
    clean(
        data,
        truthy(header),
        delim,
        out_delim,
        truthy(trim),
        truthy(dedupe),
        truthy(drop_empty_rows),
        empties,
        fill_value,
        le,
    )
    .map_err(|e| JsValue::from_str(&e))
}
