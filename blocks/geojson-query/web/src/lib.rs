//! Browser-facing wasm-bindgen wrapper for /tools/geojson-query/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    input: &str,
    query: &str,
    bbox: &str,
    format: &str,
    indent: &str,
    include_bbox: &str,
) -> Result<String, JsValue> {
    let truthy = matches!(
        include_bbox.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    );
    gizza_ai_geojson_query_core::query(
        input,
        query,
        bbox,
        format,
        indent.trim().parse::<i64>().unwrap_or(0),
        truthy,
    )
    .map_err(|e| JsValue::from_str(&e))
}
