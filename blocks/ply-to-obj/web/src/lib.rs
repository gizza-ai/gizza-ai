//! Browser-facing wasm-bindgen wrapper for /tools/ply-to-obj/.
//! Field order MUST match meta.toml: ply, input_format, colors, normals, uvs,
//! triangulate, scale, axis, name, output.
//! Every field arrives as a string (the page passes raw strings, no coercion),
//! so `scale` is parsed and the checkboxes are read positive-truthy here; the
//! rest are validated inside `core`.
use gizza_ai_ply_to_obj_core::{convert, Axis, InputFormat, Options, Output};
use wasm_bindgen::prelude::*;

/// A page checkbox arrives as "true"/"false"; accept the other common truthy
/// spellings so a hand-written `?colors=1` deep link behaves the same.
fn truthy(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    ply: &str,
    input_format: &str,
    colors: &str,
    normals: &str,
    uvs: &str,
    triangulate: &str,
    scale: &str,
    axis: &str,
    name: &str,
    output: &str,
) -> Result<String, JsValue> {
    // Blank scale → 1.0 (unchanged); a non-numeric value is a clear error.
    let scale = match scale.trim() {
        "" => 1.0,
        s => s
            .parse::<f64>()
            .map_err(|_| JsValue::from_str(&format!("scale '{s}' is not a number")))?,
    };
    let opt = Options {
        input_format: InputFormat::parse(input_format).map_err(|e| JsValue::from_str(&e))?,
        colors: truthy(colors),
        normals: truthy(normals),
        uvs: truthy(uvs),
        triangulate: truthy(triangulate),
        scale,
        axis: Axis::parse(axis).map_err(|e| JsValue::from_str(&e))?,
        name: if name.trim().is_empty() {
            "mesh".to_string()
        } else {
            name.to_string()
        },
        output: Output::parse(output).map_err(|e| JsValue::from_str(&e))?,
    };
    convert(ply, &opt).map_err(|e| JsValue::from_str(&e))
}
