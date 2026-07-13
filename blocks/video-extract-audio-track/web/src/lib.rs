//! Browser-facing wasm-bindgen wrapper for /tools/video-extract-audio-track/
//! (ffmpeg page). Builds the ffmpeg argv (pure, shared with the chat block via
//! core); the JS page driver runs it through the browser ffmpeg bridge.
//!
//! Page field order (meta.toml) MUST match this param order: `container`, then
//! `track`, then the file (`in_name`). `tool.js` calls
//! `build_argv(...fieldArgs, inName)`.

use gizza_ai_block_utils::ArgvPlan;
use gizza_ai_video_extract_audio_track_core::plan;
use wasm_bindgen::prelude::*;

/// `container` is `mka|m4a|ogg` (empty defaults to mka); `track` is the audio
/// stream index (0 = first). Returns `{ argv, out_name }` or throws a JS error
/// string.
#[wasm_bindgen]
pub fn build_argv(container: &str, track: f64, in_name: &str) -> Result<JsValue, JsValue> {
    let idx = if track.is_finite() && track > 0.0 {
        track.round() as u32
    } else {
        0
    };
    let (argv, out_name) = plan(container, idx, in_name).map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
