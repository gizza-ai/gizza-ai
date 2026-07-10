//! Browser-facing wasm-bindgen wrapper for /tools/video-audio-denoise/ (ffmpeg
//! page). Builds the ffmpeg argv (pure, shared with the chat block via core);
//! the JS page driver runs it through the browser ffmpeg bridge.
//!
//! Page field order (meta.toml) MUST match this param order: `strength`, then
//! `method`, then `remove_hum`, then the file (`in_name`). `tool.js` calls
//! `build_argv(...fieldArgs, inName)`.
use gizza_ai_block_utils::ArgvPlan;
use gizza_ai_video_audio_denoise_core::plan;
use wasm_bindgen::prelude::*;

/// An empty strength field arrives as `0.0`, which is below the accepted range
/// (1–100) — treat it as the `12` placeholder so the leave-it-blank flow uses a
/// sensible conservative default, matching the page placeholder. `method` is
/// `afftdn|anlmdn` (empty → afftdn); `remove_hum` is the checkbox value string
/// (positive truthy). Returns `{ argv, out_name }` or throws a JS error string.
#[wasm_bindgen]
pub fn build_argv(
    strength: f64,
    method: &str,
    remove_hum: &str,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    let s = if strength == 0.0 { 12.0 } else { strength };
    let hum_on = matches!(remove_hum, "true" | "1" | "on" | "yes");
    let (argv, out_name) = plan(in_name, s, method, hum_on).map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
