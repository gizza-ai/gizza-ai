//! Browser-facing wasm-bindgen wrapper for /tools/geojson-coords-to-csv/.
//! Field order MUST match page/meta.toml: geojson, order, columns, shapes,
//! elevation, precision, dedupe, properties, delimiter, header. The page passes
//! every field as a string, so booleans arrive as "true"/"false" and the numeric
//! `precision` arrives as text — both are parsed here.
use gizza_ai_geojson_coords_to_csv_core::convert_str;
use wasm_bindgen::prelude::*;

/// Page checkboxes marshal as "true"/"false"; accept the other positive forms too.
fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    geojson: &str,
    order: &str,
    columns: &str,
    shapes: &str,
    elevation: &str,
    precision: &str,
    dedupe: &str,
    properties: &str,
    delimiter: &str,
    header: &str,
) -> Result<String, JsValue> {
    let precision = match precision.trim() {
        "" => -1,
        other => other.parse::<i32>().map_err(|_| {
            JsValue::from_str(&format!(
                "invalid precision '{other}': expected -1 (keep the source value) or 0-15 decimal places"
            ))
        })?,
    };
    convert_str(
        geojson,
        order,
        columns,
        shapes,
        elevation,
        precision,
        dedupe,
        truthy(properties),
        delimiter,
        truthy(header),
    )
    .map_err(|e| JsValue::from_str(&e))
}
