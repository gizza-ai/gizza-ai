//! Browser-facing wasm-bindgen wrapper for /tools/geojson-merge/.
//! Field order MUST match meta.toml: geojson, indent, precision, renumber, source_property.
//! tool.js passes every field as a raw string, so parse each here.
use gizza_ai_geojson_merge_core::merge_geojson;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    geojson: &str,
    indent: &str,
    precision: &str,
    renumber: &str,
    source_property: &str,
) -> Result<String, JsValue> {
    let indent: usize = indent.trim().parse().unwrap_or(2);
    // Blank precision field means "keep full precision" (-1 sentinel).
    let precision: i64 = match precision.trim() {
        "" => -1,
        s => s.parse().unwrap_or(-1),
    };
    let renumber = matches!(
        renumber.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    );
    merge_geojson(geojson, indent, precision, renumber, source_property)
        .map_err(|e| JsValue::from_str(&e))
}
