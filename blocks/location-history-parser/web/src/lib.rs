//! Browser-facing wasm-bindgen wrapper for /tools/location-history-parser/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    input: &str,
    output: &str,
    unit: &str,
    utc_offset: &str,
    place_radius: &str,
    min_stay: &str,
) -> Result<String, JsValue> {
    let utc_offset = utc_offset.trim().parse::<f64>().unwrap_or(0.0);
    let place_radius = parse_or(place_radius, 150.0);
    let min_stay = parse_or(min_stay, 10.0);
    gizza_ai_location_history_parser_core::run(
        input,
        empty_default(output, "text"),
        empty_default(unit, "km"),
        utc_offset,
        place_radius,
        min_stay,
    )
    .map_err(|e| JsValue::from_str(&e))
}

fn empty_default<'a>(s: &'a str, d: &'a str) -> &'a str {
    if s.trim().is_empty() {
        d
    } else {
        s
    }
}
fn parse_or(s: &str, d: f64) -> f64 {
    match s.trim().parse::<f64>() {
        Ok(v) => v,
        Err(_) => d,
    }
}
