//! Browser-facing wasm-bindgen wrapper for /tools/video-audio-to-mono/ (ffmpeg
//! page). Builds the ffmpeg argv (pure, shared with the chat block via core);
//! the JS page driver runs it through the browser ffmpeg bridge.
//!
//! Page field order (meta.toml) MUST match this param order: `channel`, then
//! `bitrate`, then `sample_rate`, then the file (`in_name`). `tool.js` calls
//! `build_argv(...fieldArgs, inName)`.
use gizza_ai_block_utils::ArgvPlan;
use gizza_ai_video_audio_to_mono_core::plan;
use wasm_bindgen::prelude::*;

/// `channel` is `mix|left|right|difference` (empty → mix). `bitrate` is kbps;
/// an empty number field arrives as `0.0`, which is below the 16 kbps floor, so
/// treat it as the 128 default the page placeholder advertises. `sample_rate`
/// is the enum string (empty → keep). Returns `{ argv, out_name }` or throws a
/// JS error string.
#[wasm_bindgen]
pub fn build_argv(
    channel: &str,
    bitrate: f64,
    sample_rate: &str,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    let kbps = if bitrate == 0.0 { 128.0 } else { bitrate };
    let (argv, out_name) =
        plan(in_name, channel, kbps, sample_rate).map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
