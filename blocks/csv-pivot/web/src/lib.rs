//! Browser-facing wasm-bindgen wrapper for /tools/csv-pivot/.
use gizza_ai_csv_pivot_core::pivot;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(data: &str, rows: &str, columns: &str, values: &str, agg: &str, delimiter: &str) -> Result<String, JsValue> {
    let agg = if agg.is_empty() { "sum" } else { agg };
    let delim = if delimiter.is_empty() { "," } else { delimiter };
    pivot(data, rows, columns, values, agg, true, delim).map_err(|e| JsValue::from_str(&e))
}
