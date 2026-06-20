//! Browser-facing wasm-bindgen wrapper for /tools/csv-json-convert/.
//!
//! tool.js passes EVERY page field as a raw string (no coercion for pure tools),
//! so this export takes `&str` for every param and parses the bool/enum fields
//! here; the core owns all validation. Param order MUST match page/meta.toml's
//! [[input]] order.
use gizza_ai_csv_json_convert_core::{convert, Direction};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    data: &str,
    direction: &str,
    delimiter: &str,
    headers: &str,
    pretty: &str,
    flatten: &str,
) -> Result<String, JsValue> {
    let dir = Direction::parse(direction).map_err(|e| JsValue::from_str(&e))?;
    // Blank header field defaults to true (the common csv→json case); pretty and
    // flatten default to false. Accept "true"/"1"/"on" as truthy.
    let headers = parse_bool(headers, true);
    let pretty = parse_bool(pretty, false);
    let flatten = parse_bool(flatten, false);
    convert(data, dir, delimiter, headers, pretty, flatten).map_err(|e| JsValue::from_str(&e))
}

fn parse_bool(s: &str, default: bool) -> bool {
    match s.trim().to_ascii_lowercase().as_str() {
        "" => default,
        "true" | "1" | "on" | "yes" => true,
        _ => false,
    }
}
