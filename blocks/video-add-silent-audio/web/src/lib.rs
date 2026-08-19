//! Browser-facing wasm-bindgen wrapper for /tools/video-add-silent-audio/
//! (ffmpeg page). Builds the ffmpeg argv (pure, shared with the chat block via
//! core); the JS page driver runs it through the browser ffmpeg bridge.
//!
//! Page field order (meta.toml) MUST match this param order: `channels`,
//! `sample_rate`, `bitrate`, `existing_audio`, then the file (`in_name`).
//! `tool.js` calls `build_argv(...fieldArgs, inName)`. Every field is an enum
//! `<select>` value string; an empty value lets core apply its default.
use gizza_ai_block_utils::ArgvPlan;
use gizza_ai_video_add_silent_audio_core::plan;
use wasm_bindgen::prelude::*;

/// Returns `{ argv, out_name }` or throws a JS error string describing which
/// option was invalid.
#[wasm_bindgen]
pub fn build_argv(
    channels: &str,
    sample_rate: &str,
    bitrate: &str,
    existing_audio: &str,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    let (argv, out_name) = plan(in_name, channels, sample_rate, bitrate, existing_audio)
        .map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
