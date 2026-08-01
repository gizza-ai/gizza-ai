//! Browser-facing wasm-bindgen wrapper for the standalone /tools/video-timelapse/
//! page. Builds the ffmpeg argv (pure, shared with the chat block via core); the
//! JS page driver runs it through the browser ffmpeg bridge.
use gizza_ai_block_utils::ArgvPlan;
use wasm_bindgen::prelude::*;

/// `speed`/`fps` of 0 (or non-finite) mean "unset" and fall back to the defaults
/// (10× / 30 fps); the values are clamped to the accepted ranges by core.
/// Returns `{ argv: string[], out_name }`. The page passes the `speed` then
/// `fps` fields, then the uploaded file's `in_name` (field order = `build_argv`
/// param order).
#[wasm_bindgen]
pub fn build_argv(speed: f64, fps: f64, in_name: &str) -> Result<JsValue, JsValue> {
    let (argv, out_name) = gizza_ai_video_timelapse_core::build_argv(speed, fps, in_name);
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
