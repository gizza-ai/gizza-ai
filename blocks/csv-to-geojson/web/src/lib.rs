//! Browser-facing wasm-bindgen wrapper for /tools/csv-to-geojson/.
use gizza_ai_csv_to_geojson_core::{convert, Options};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    input: &str,
    lat: &str,
    lon: &str,
    elevation: &str,
    delimiter: &str,
    shape: &str,
    types: &str,
    precision: &str,
    invalid: &str,
    bbox: &str,
    pretty: &str,
) -> Result<String, JsValue> {
    let d = Options::default();
    let precision = if precision.trim().is_empty() {
        d.precision
    } else {
        precision.trim().parse::<i64>().map_err(|_| {
            JsValue::from_str(&format!(
                "precision must be a whole number, got {precision:?}"
            ))
        })?
    };
    convert(
        input,
        &Options {
            lat: lat.to_string(),
            lon: lon.to_string(),
            elevation: elevation.to_string(),
            delimiter: if delimiter.trim().is_empty() {
                d.delimiter
            } else {
                delimiter.to_string()
            },
            shape: if shape.trim().is_empty() {
                d.shape
            } else {
                shape.to_string()
            },
            types: if types.trim().is_empty() {
                d.types
            } else {
                types.to_string()
            },
            precision,
            invalid: if invalid.trim().is_empty() {
                d.invalid
            } else {
                invalid.to_string()
            },
            bbox: flag(bbox),
            pretty: if pretty.trim().is_empty() {
                d.pretty
            } else {
                flag(pretty)
            },
        },
    )
    .map_err(|e| JsValue::from_str(&e))
}

fn flag(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}
