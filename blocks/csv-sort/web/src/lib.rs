//! Browser-facing wasm-bindgen wrapper for /tools/csv-sort/.
//!
//! The standalone page passes every field value as a string, so the
//! boolean params (`case_sensitive`, `header`) arrive as strings and are
//! parsed here. `case_sensitive` defaults OFF (blank → false); `header`
//! defaults ON (blank → true, only an explicit false/0/off/no turns it off).
use gizza_ai_csv_sort_core::sort;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    data: &str,
    columns: &str,
    order: &str,
    numeric: &str,
    case_sensitive: &str,
    header: &str,
    delimiter: &str,
) -> Result<String, JsValue> {
    let case_sensitive = matches!(
        case_sensitive.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    );
    let header = !matches!(header.trim().to_ascii_lowercase().as_str(), "false" | "0" | "off" | "no");
    let delim = if delimiter.is_empty() { "," } else { delimiter };
    sort(data, columns, order, numeric, case_sensitive, header, delim)
        .map_err(|e| JsValue::from_str(&e))
}
