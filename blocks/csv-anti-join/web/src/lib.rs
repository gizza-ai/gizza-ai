//! Browser-facing wasm-bindgen wrapper for /tools/csv-anti-join/.
//!
//! The standalone page passes every field value as a string, so the boolean
//! `case_sensitive`/`trim_keys` params arrive as strings and are parsed here.
//! `case_sensitive` defaults ON (blank → true); `trim_keys` defaults OFF.
use gizza_ai_csv_anti_join_core::anti_join;
use wasm_bindgen::prelude::*;

fn truthy(v: &str) -> bool {
    matches!(v.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
pub fn run(
    a: &str,
    b: &str,
    key: &str,
    key_b: &str,
    direction: &str,
    delimiter: &str,
    case_sensitive: &str,
    trim_keys: &str,
) -> Result<String, JsValue> {
    // case_sensitive defaults ON: blank stays true, only an explicit falsey turns it off.
    let case_sensitive = !matches!(
        case_sensitive.trim().to_ascii_lowercase().as_str(),
        "false" | "0" | "off" | "no"
    );
    let trim_keys = truthy(trim_keys);
    let delim = if delimiter.is_empty() { "," } else { delimiter };
    let dir = if direction.is_empty() { "a-only" } else { direction };
    anti_join(a, b, key, key_b, dir, delim, case_sensitive, trim_keys)
        .map_err(|e| JsValue::from_str(&e))
}
