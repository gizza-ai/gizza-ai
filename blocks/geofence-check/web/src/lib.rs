//! Browser-facing wasm-bindgen wrapper for /tools/geofence-check/.
//! The standalone page passes every field value as a string; enum defaults are
//! applied here when a field is empty (never touched on the page).
use gizza_ai_geofence_check_core::check;
use wasm_bindgen::prelude::*;

/// Test whether latitude/longitude `points` fall inside `polygon`.
///
/// - `coord_order`: `"lat_lon"` (default) | `"lon_lat"` — for the non-GeoJSON forms.
/// - `boundary`: `"inside"` (default) | `"outside"` | `"boundary"`.
/// - `output`: `"text"` (default) | `"csv"` | `"json"`.
#[wasm_bindgen]
pub fn run(
    polygon: &str,
    points: &str,
    coord_order: &str,
    boundary: &str,
    output: &str,
) -> Result<String, JsValue> {
    let coord_order = if coord_order.trim().is_empty() {
        "lat_lon"
    } else {
        coord_order
    };
    let boundary = if boundary.trim().is_empty() {
        "inside"
    } else {
        boundary
    };
    let output = if output.trim().is_empty() {
        "text"
    } else {
        output
    };
    check(polygon, points, coord_order, boundary, output).map_err(|e| JsValue::from_str(&e))
}
