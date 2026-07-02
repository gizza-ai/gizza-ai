//! Browser-facing wasm-bindgen wrapper for /tools/audio-compress/ (ffmpeg page).
//! Builds the ffmpeg argv (pure, shared with the chat block via core); the JS
//! page driver runs it through the browser ffmpeg bridge.
//!
//! Page field order (meta.toml) MUST match this param order: `bitrate`, then
//! `format`, then the file (`in_name`). `tool.js` calls
//! `build_argv(...fieldArgs, inName)`.

use wasm_bindgen::prelude::*;

use gizza_ai_audio_compress_core::{plan_compress, DEFAULT_BITRATE};
use gizza_ai_block_utils::ArgvPlan;

/// `bitrate` is the target kbps (empty/0 defaults to 96; out-of-range values
/// throw — rejected, not clamped); `format` is `mp3|ogg|m4a` (empty defaults
/// to mp3). Returns `{ argv, out_name }` or throws an error string.
#[wasm_bindgen]
pub fn build_argv(bitrate: f64, format: &str, in_name: &str) -> Result<JsValue, JsValue> {
    let kbps = if bitrate == 0.0 {
        DEFAULT_BITRATE
    } else {
        // Negative/NaN saturate to 0 in the cast and are rejected by the plan.
        bitrate.round() as u32
    };
    let (argv, out_name) =
        plan_compress(in_name, format, kbps).map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
