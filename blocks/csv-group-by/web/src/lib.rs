//! Browser-facing wasm-bindgen wrapper for /tools/csv-group-by/.
use gizza_ai_csv_group_by_core::group_by;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(data: &str, group_by_cols: &str, aggregations: &str, delimiter: &str) -> Result<String, JsValue> {
    let delim = if delimiter.is_empty() { "," } else { delimiter };
    group_by(data, group_by_cols, aggregations, true, delim).map_err(|e| JsValue::from_str(&e))
}
