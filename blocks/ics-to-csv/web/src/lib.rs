//! Browser-facing wasm-bindgen wrapper for /tools/ics-to-csv/.
//!
//! tool.js passes EVERY page field as a raw string (no coercion for pure tools),
//! so this export takes `&str` for every param and converts the checkbox
//! (boolean) fields here; the core owns all validation. Param order MUST match
//! page/meta.toml's [[input]] order.
use gizza_ai_ics_to_csv_core::convert_str;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    ics: &str,
    delimiter: &str,
    header: &str,
    date_format: &str,
    include_location: &str,
    include_description: &str,
) -> Result<String, JsValue> {
    convert_str(
        ics,
        delimiter,
        parse_bool(header),
        date_format,
        parse_bool(include_location),
        parse_bool(include_description),
    )
    .map_err(|e| JsValue::from_str(&e))
}

fn parse_bool(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}
