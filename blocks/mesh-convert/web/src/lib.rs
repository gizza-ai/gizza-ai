//! Browser-facing wasm-bindgen wrapper for /tools/mesh-convert/.
//! Field order MUST match meta.toml: mesh, to, stl_encoding, scale, axis, name.
//! Every field arrives as a string (the page passes raw strings, no coercion),
//! so `scale` is parsed here; the rest are validated inside `core`.
use gizza_ai_mesh_convert_core::{convert, Axis, Options, StlEncoding, Target};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    mesh: &str,
    to: &str,
    stl_encoding: &str,
    scale: &str,
    axis: &str,
    name: &str,
) -> Result<String, JsValue> {
    // Blank scale → 1.0 (unchanged); a non-numeric value is a clear error.
    let scale = match scale.trim() {
        "" => 1.0,
        s => s
            .parse::<f64>()
            .map_err(|_| JsValue::from_str(&format!("scale '{s}' is not a number")))?,
    };
    let opt = Options {
        to: Target::parse(to).map_err(|e| JsValue::from_str(&e))?,
        stl_encoding: StlEncoding::parse(stl_encoding).map_err(|e| JsValue::from_str(&e))?,
        scale,
        axis: Axis::parse(axis).map_err(|e| JsValue::from_str(&e))?,
        name: if name.trim().is_empty() {
            "mesh".to_string()
        } else {
            name.to_string()
        },
    };
    convert(mesh, &opt).map_err(|e| JsValue::from_str(&e))
}
