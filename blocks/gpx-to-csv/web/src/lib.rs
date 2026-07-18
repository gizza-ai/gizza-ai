//! Browser-facing wasm-bindgen wrapper for /tools/gpx-to-csv/.
//!
//! tool.js passes EVERY page field as a raw string (no coercion for pure tools),
//! so this export takes `&str` for every param and converts the checkbox
//! (boolean) fields here; the core owns all validation. Param order MUST match
//! page/meta.toml's [[input]] order.
use gizza_ai_gpx_to_csv_core::convert_str;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    gpx: &str,
    points: &str,
    delimiter: &str,
    header: &str,
    time_format: &str,
    speed: &str,
) -> Result<String, JsValue> {
    convert_str(
        gpx,
        points,
        delimiter,
        parse_bool(header),
        time_format,
        parse_bool(speed),
    )
    .map_err(|e| JsValue::from_str(&e))
}

fn parse_bool(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}
