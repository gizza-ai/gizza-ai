//! Browser-facing wasm-bindgen wrapper for /tools/obj-to-gltf/.
//! Field order MUST match page/meta.toml: obj, mtl, to, up_axis, scale,
//! normals, name, unlit, double_sided.
use wasm_bindgen::prelude::*;

fn truthy(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

fn parse_scale(value: &str) -> Result<f64, JsValue> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(1.0);
    }
    trimmed
        .parse::<f64>()
        .map_err(|_| JsValue::from_str(&format!("scale must be a number, got '{trimmed}'")))
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    obj: &str,
    mtl: &str,
    to: &str,
    up_axis: &str,
    scale: &str,
    normals: &str,
    name: &str,
    unlit: &str,
    double_sided: &str,
) -> Result<String, JsValue> {
    gizza_ai_obj_to_gltf_core::run(
        obj,
        mtl,
        to,
        up_axis,
        parse_scale(scale)?,
        normals,
        name,
        truthy(unlit),
        truthy(double_sided),
    )
    .map_err(|e| JsValue::from_str(&e))
}
