//! Browser-facing wasm-bindgen wrapper for /tools/obj-vertices-to-csv/.
//! Field order MUST match page/meta.toml: obj, columns, color, up_axis, scale,
//! precision, dedupe, objects, delimiter, header. The page passes every field as
//! a string, so the checkbox arrives as "true"/"false" and the numeric `scale`
//! and `precision` arrive as text — all three are parsed here.
use gizza_ai_obj_vertices_to_csv_core::convert_str;
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
    obj: &str,
    columns: &str,
    color: &str,
    up_axis: &str,
    scale: &str,
    precision: &str,
    dedupe: &str,
    objects: &str,
    delimiter: &str,
    header: &str,
) -> Result<String, JsValue> {
    let scale = match scale.trim() {
        "" => 1.0,
        other => other.parse::<f64>().map_err(|_| {
            JsValue::from_str(&format!(
                "invalid scale '{other}': expected a number (1 keeps the source size, 1000 = \
                 metres to millimetres)"
            ))
        })?,
    };
    let precision = match precision.trim() {
        "" => -1,
        other => other.parse::<i32>().map_err(|_| {
            JsValue::from_str(&format!(
                "invalid precision '{other}': expected -1 (keep the source value) or 0-15 decimal \
                 places"
            ))
        })?,
    };
    convert_str(
        obj,
        columns,
        color,
        up_axis,
        scale,
        precision,
        dedupe,
        objects,
        delimiter,
        truthy(header),
    )
    .map_err(|e| JsValue::from_str(&e))
}
