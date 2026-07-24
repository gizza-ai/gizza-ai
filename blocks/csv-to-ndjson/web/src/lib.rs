//! Browser-facing wasm-bindgen wrapper for /tools/csv-to-ndjson/.
//!
//! tool.js passes EVERY page field as a raw string (no coercion for pure tools),
//! so this export takes `&str` for every param and parses the bool fields here;
//! the core owns all validation. Param order MUST match page/meta.toml's
//! [[input]] order.
use gizza_ai_csv_to_ndjson_core::to_ndjson;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    data: &str,
    delimiter: &str,
    headers: &str,
    parse_numbers: &str,
    parse_bools: &str,
    trim: &str,
) -> Result<String, JsValue> {
    // headers defaults true (the common case); the inference/trim flags default
    // false. Accept "true"/"1"/"on"/"yes" as truthy.
    let headers = parse_bool(headers, true);
    let parse_numbers = parse_bool(parse_numbers, false);
    let parse_bools = parse_bool(parse_bools, false);
    let trim = parse_bool(trim, false);
    to_ndjson(data, delimiter, headers, parse_numbers, parse_bools, trim)
        .map_err(|e| JsValue::from_str(&e))
}

fn parse_bool(s: &str, default: bool) -> bool {
    match s.trim().to_ascii_lowercase().as_str() {
        "" => default,
        "true" | "1" | "on" | "yes" => true,
        _ => false,
    }
}
