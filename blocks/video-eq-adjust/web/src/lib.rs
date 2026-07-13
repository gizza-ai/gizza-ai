//! Browser-facing wasm-bindgen wrapper for the standalone /tools/video-eq-adjust/
//! page. Builds the ffmpeg argv (pure, shared with the chat block via core); the
//! JS page driver runs it through the browser ffmpeg bridge.
//!
//! The page passes the field values then the uploaded file's `in_name` (field
//! order in page/meta.toml MUST equal this param order). Values are coerced to
//! `Number` by the page for numeric schema params; the shared `plan()` validates
//! ranges and rejects out-of-range values with a clear error.
use gizza_ai_block_utils::ArgvPlan;
use gizza_ai_video_eq_adjust_core::plan;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn build_argv(
    brightness: f64,
    contrast: f64,
    saturation: f64,
    gamma: f64,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    let (argv, out_name) =
        plan(in_name, brightness, contrast, saturation, gamma).map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
