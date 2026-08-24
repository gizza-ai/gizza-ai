//! Browser-facing wasm-bindgen wrapper for /tools/stl-vertices-to-csv/.
//! Field order MUST match page/meta.toml: stl, input_format, rows, columns,
//! normal_source, up_axis, scale, precision, dedupe, every_nth, delimiter,
//! header. The page passes every field as a string, so the checkbox arrives as
//! "true"/"false" and the three numeric fields arrive as text — all are parsed
//! here.
use gizza_ai_stl_vertices_to_csv_core::convert_str;
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
    stl: &str,
    input_format: &str,
    rows: &str,
    columns: &str,
    normal_source: &str,
    up_axis: &str,
    scale: &str,
    precision: &str,
    dedupe: &str,
    every_nth: &str,
    delimiter: &str,
    header: &str,
) -> Result<String, JsValue> {
    let scale = match scale.trim() {
        "" => 1.0,
        other => other.parse::<f64>().map_err(|_| {
            JsValue::from_str(&format!(
                "invalid scale '{other}': expected a number (1 keeps the source size, 25.4 = \
                 inches to millimetres)"
            ))
        })?,
    };
    let precision = match precision.trim() {
        "" => -1,
        other => other.parse::<i32>().map_err(|_| {
            JsValue::from_str(&format!(
                "invalid precision '{other}': expected -1 (shortest round-tripping text) or 0-15 \
                 decimal places"
            ))
        })?,
    };
    let every_nth = match every_nth.trim() {
        "" => 1,
        other => other.parse::<i64>().map_err(|_| {
            JsValue::from_str(&format!(
                "invalid every_nth '{other}': expected a whole number of rows to step by (1 keeps \
                 every row)"
            ))
        })?,
    };
    convert_str(
        stl,
        input_format,
        rows,
        columns,
        normal_source,
        up_axis,
        scale,
        precision,
        dedupe,
        every_nth,
        delimiter,
        truthy(header),
    )
    .map_err(|e| JsValue::from_str(&e))
}
