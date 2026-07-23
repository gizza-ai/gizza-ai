//! Browser-facing wasm-bindgen wrapper for /tools/social-image-resize/.
//! Builds an ffmpeg argv plan for the generated page runtime.
use gizza_ai_block_utils::ArgvPlan;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn build_argv(
    target: &str,
    fit: &str,
    gravity: &str,
    background: &str,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    let (argv, out_name) = gizza_ai_social_image_resize_core::plan(
        in_name,
        target,
        Some(fit),
        Some(gravity),
        background,
    )
    .map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
