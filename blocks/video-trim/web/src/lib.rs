//! Browser-facing wasm-bindgen wrapper for /tools/video-trim/ (ffmpeg page).
//! Builds the ffmpeg argv (pure, shared with the chat block via core); the JS
//! page driver runs it through the browser ffmpeg bridge.
//!
//! Page field order (meta.toml) MUST match this param order: `start`, then
//! `duration`, then the file (`in_name`). `tool.js` calls
//! `build_argv(...fieldArgs, inName)`.

use wasm_bindgen::prelude::*;

use gizza_ai_block_utils::ArgvPlan;
use gizza_ai_video_trim_core::plan_trim;

/// `start` is the trim start in seconds (>= 0); `duration` is the clip length in
/// seconds (> 0). Empty page fields arrive here as `0.0` — start=0 means "from
/// the beginning", but duration=0 is invalid and surfaces as a JS error string.
/// Returns `{ argv: string[], out_name }` (out is always `out.mp4`) or throws.
#[wasm_bindgen]
pub fn build_argv(start: f64, duration: f64, in_name: &str) -> Result<JsValue, JsValue> {
    let (argv, out_name) =
        plan_trim(in_name, start, duration).map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
