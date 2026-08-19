//! Browser-facing wasm-bindgen wrapper for /tools/geo-cluster/.
//! Field order MUST match meta.toml: points, method, radius, units, min_points,
//! coord_order, output. Every field arrives as a string.
use gizza_ai_geo_cluster_core::cluster;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    points: &str,
    method: &str,
    radius: &str,
    units: &str,
    min_points: &str,
    coord_order: &str,
    output: &str,
) -> Result<String, JsValue> {
    let radius: f64 = match radius.trim() {
        "" => 500.0,
        s => s
            .parse()
            .map_err(|_| JsValue::from_str(&format!("radius '{s}' is not a number")))?,
    };
    let min_points: f64 = match min_points.trim() {
        "" => 2.0,
        s => s
            .parse()
            .map_err(|_| JsValue::from_str(&format!("min_points '{s}' is not a number")))?,
    };
    cluster(
        points,
        method,
        radius,
        units,
        min_points,
        coord_order,
        output,
    )
    .map_err(|e| JsValue::from_str(&e))
}
