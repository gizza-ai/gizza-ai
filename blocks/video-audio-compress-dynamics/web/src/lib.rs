//! Browser-facing wasm-bindgen wrapper for /tools/video-audio-compress-dynamics/
//! (ffmpeg page). Builds the ffmpeg argv (pure, shared with the chat block via
//! core); the JS page driver runs it through the browser ffmpeg bridge.
//!
//! Page field order (meta.toml) MUST match this param order: `preset`, then
//! `makeup`, then the file (`in_name`). `tool.js` calls
//! `build_argv(...fieldArgs, inName)`.
use gizza_ai_block_utils::ArgvPlan;
use gizza_ai_video_audio_compress_dynamics_core::plan;
use wasm_bindgen::prelude::*;

/// `preset` is `light|medium|heavy` (empty → medium). `makeup` is the checkbox
/// value string (positive truthy). Returns `{ argv, out_name }` or throws a JS
/// error string.
#[wasm_bindgen]
pub fn build_argv(preset: &str, makeup: &str, in_name: &str) -> Result<JsValue, JsValue> {
    let makeup_on = matches!(makeup, "true" | "1" | "on" | "yes");
    let (argv, out_name) = plan(in_name, preset, makeup_on).map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
