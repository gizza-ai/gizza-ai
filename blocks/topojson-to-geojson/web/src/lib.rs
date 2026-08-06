//! Browser-facing wasm-bindgen wrapper for /tools/topojson-to-geojson/.
//! Field order MUST match meta.toml: topojson, object, output, include_bbox,
//! precision, indent. Fields arrive as strings; checkboxes as "true"/"false".
use gizza_ai_topojson_to_geojson_core::{topojson_to_geojson, Options, Output};
use wasm_bindgen::prelude::*;

fn truthy(v: &str) -> bool {
    matches!(v.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
pub fn run(
    topojson: &str,
    object: &str,
    output: &str,
    include_bbox: &str,
    precision: &str,
    indent: &str,
) -> Result<String, JsValue> {
    let opts = Options {
        object: object.to_string(),
        output: Output::parse(output),
        include_bbox: truthy(include_bbox),
        precision: precision.trim().parse().unwrap_or(-1),
        indent: indent.trim().parse().unwrap_or(2),
    };
    topojson_to_geojson(topojson, &opts).map_err(|e| JsValue::from_str(&e))
}
