//! Browser-facing wasm-bindgen wrapper for /tools/geojson-to-svg/.
use gizza_ai_geojson_to_svg_core::{render, Options};
use wasm_bindgen::prelude::*;

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    geojson: &str,
    width: &str,
    fill: &str,
    stroke: &str,
    stroke_width: &str,
    point_radius: &str,
    background: &str,
    projection: &str,
) -> Result<String, JsValue> {
    let width = if width.trim().is_empty() {
        800.0
    } else {
        width.trim().parse::<f64>().map_err(|_| JsValue::from_str("width must be a number"))?
    };
    let stroke_width = if stroke_width.trim().is_empty() {
        1.0
    } else {
        stroke_width
            .trim()
            .parse::<f64>()
            .map_err(|_| JsValue::from_str("stroke_width must be a number"))?
    };
    let point_radius = if point_radius.trim().is_empty() {
        4.0
    } else {
        point_radius
            .trim()
            .parse::<f64>()
            .map_err(|_| JsValue::from_str("point_radius must be a number"))?
    };
    let fill = if fill.trim().is_empty() { "#cfe8ff" } else { fill };
    let stroke = if stroke.trim().is_empty() { "#1f6feb" } else { stroke };
    let background = if background.trim().is_empty() { "#ffffff" } else { background };

    let opt = Options {
        width,
        padding: 16.0,
        fill: fill.to_string(),
        stroke: stroke.to_string(),
        stroke_width,
        point_radius,
        background: background.to_string(),
        mercator: projection != "none",
    };
    render(geojson, &opt).map_err(|e| JsValue::from_str(&e))
}
