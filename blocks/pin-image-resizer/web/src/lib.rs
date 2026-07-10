//! Browser-facing wasm-bindgen wrapper for /tools/pin-image-resizer/ (ffmpeg page).
//! Builds the ffmpeg argv (pure, shared with the chat block via core); the JS
//! page driver runs it through the browser ffmpeg bridge. Field order in
//! page/meta.toml MUST match this param order: preset, fit, gravity, background.
use gizza_ai_block_utils::ArgvPlan;
use gizza_ai_pin_image_resizer_core::plan;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn build_argv(
    preset: &str,
    fit: &str,
    gravity: &str,
    background: &str,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    let (argv, out_name) = plan(in_name, preset, Some(fit), Some(gravity), background)
        .map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
