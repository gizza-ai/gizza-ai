//! Browser-facing wasm-bindgen wrapper for /tools/csv-join/.
//!
//! The standalone page passes every field value as a string, so the boolean
//! `case_sensitive` param arrives as a string and is parsed here. It defaults ON
//! (blank → true); only an explicit false/0/off/no turns it off.
use gizza_ai_csv_join_core::join;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    left: &str,
    right: &str,
    left_key: &str,
    right_key: &str,
    join_type: &str,
    delimiter: &str,
    case_sensitive: &str,
) -> Result<String, JsValue> {
    let case_sensitive = !matches!(
        case_sensitive.trim().to_ascii_lowercase().as_str(),
        "false" | "0" | "off" | "no"
    );
    let delim = if delimiter.is_empty() { "," } else { delimiter };
    let jt = if join_type.is_empty() { "inner" } else { join_type };
    join(left, right, left_key, right_key, jt, delim, case_sensitive)
        .map_err(|e| JsValue::from_str(&e))
}
