//! Browser-facing wasm-bindgen wrapper for /tools/utm-to-latlon-csv/.
//!
//! tool.js passes pure tool page fields as strings, so booleans and integers are
//! parsed here while the core owns semantic validation.
use gizza_ai_utm_to_latlon_csv_core::convert;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    input: &str,
    direction: &str,
    easting_column: &str,
    northing_column: &str,
    zone_column: &str,
    zone: &str,
    hemisphere: &str,
    coord_format: &str,
    decimals: &str,
    ellipsoid: &str,
    delimiter: &str,
    has_header: &str,
    keep_columns: &str,
    validate_ranges: &str,
    output: &str,
) -> Result<String, JsValue> {
    let decimals = if decimals.trim().is_empty() {
        6
    } else {
        decimals
            .trim()
            .parse::<i64>()
            .map_err(|_| JsValue::from_str("decimals must be an integer between 0 and 12"))?
    };
    convert(
        input,
        direction,
        easting_column,
        northing_column,
        zone_column,
        zone,
        hemisphere,
        coord_format,
        decimals,
        ellipsoid,
        delimiter,
        parse_bool(has_header, true),
        parse_bool(keep_columns, true),
        parse_bool(validate_ranges, true),
        output,
    )
    .map_err(|e| JsValue::from_str(&e))
}

fn parse_bool(s: &str, default: bool) -> bool {
    match s.trim().to_ascii_lowercase().as_str() {
        "" => default,
        "true" | "1" | "on" | "yes" => true,
        _ => false,
    }
}
