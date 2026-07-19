//! Browser-facing wasm-bindgen wrapper for /tools/downsample-timeseries/.
//! The page passes every field as a raw string (no coercion for pure tools),
//! so numeric params arrive as &str and are parsed here (blank → default).
use gizza_ai_downsample_timeseries_core::downsample;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    data: &str,
    points: &str,
    algorithm: &str,
    x_column: &str,
    y_column: &str,
    header: &str,
    output: &str,
) -> Result<String, JsValue> {
    let points = if points.trim().is_empty() {
        100
    } else {
        points
            .trim()
            .parse::<usize>()
            .map_err(|_| JsValue::from_str("points must be a whole number between 2 and 100000"))?
    };
    // Checkbox sends "true"/"false"; blank (no field) means the default (true).
    let header = if header.trim().is_empty() {
        true
    } else {
        matches!(
            header.trim().to_ascii_lowercase().as_str(),
            "true" | "1" | "on" | "yes"
        )
    };
    downsample(data, algorithm, points, x_column, y_column, header, output)
        .map_err(|e| JsValue::from_str(&e))
}
