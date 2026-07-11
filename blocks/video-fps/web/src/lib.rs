//! Browser-facing wasm-bindgen wrapper for the standalone /tools/video-fps/
//! page. Builds the ffmpeg argv (pure, shared with the chat block via core); the
//! JS page driver runs it through the browser ffmpeg bridge.
use gizza_ai_block_utils::ArgvPlan;
use wasm_bindgen::prelude::*;

/// `fps` of 0 (or non-finite) means "unset" and falls back to the default 30;
/// the value is clamped to the accepted range by core. Returns
/// `{ argv: string[], out_name }`. The page passes the `fps` field then the
/// uploaded file's `in_name` (field order = `build_argv` param order).
#[wasm_bindgen]
pub fn build_argv(fps: f64, in_name: &str) -> Result<JsValue, JsValue> {
    let (argv, out_name) = gizza_ai_video_fps_core::build_argv(fps, in_name);
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
