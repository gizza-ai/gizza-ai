//! Browser-facing wasm-bindgen wrapper for /tools/geojson-to-csv/.
//! The standalone page passes every field value as a string, so the boolean
//! `header` param arrives as a string and is parsed here.
use gizza_ai_geojson_to_csv_core::convert_str;
use wasm_bindgen::prelude::*;

/// Flatten a GeoJSON document into CSV.
///
/// - `geometry`: `"wkt"` (default) | `"lonlat"` | `"both"` | `"none"`.
/// - `nested`: `"json"` (default) | `"flatten"`.
/// - `delimiter`: `"comma"` (default) | `"semicolon"` | `"tab"` | `"pipe"`.
/// - `header`: `"true"`/`"1"`/`"yes"`/`"on"` → header row (default on); else off.
#[wasm_bindgen]
pub fn run(
    geojson: &str,
    geometry: &str,
    nested: &str,
    delimiter: &str,
    header: &str,
) -> Result<String, JsValue> {
    // Empty header field (never touched on the page) defaults to on; an explicit
    // falsey value turns the header row off.
    let header = match header.trim().to_ascii_lowercase().as_str() {
        "" | "true" | "1" | "yes" | "on" => true,
        _ => false,
    };
    convert_str(geojson, geometry, nested, delimiter, header).map_err(|e| JsValue::from_str(&e))
}
