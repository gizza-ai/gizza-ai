//! Browser-facing wasm-bindgen wrapper for /tools/video-audio-bitrate-set/
//! (ffmpeg page). Builds the ffmpeg argv (pure, shared with the chat block via
//! core); the JS page driver runs it through the browser ffmpeg bridge.
//!
//! Page field order (meta.toml) MUST match this param order: `bitrate`, then the
//! file (`in_name`). `tool.js` calls `build_argv(...fieldArgs, inName)`. `bitrate`
//! is the enum `<select>` value string (empty → core defaults to 128 kbps).
use gizza_ai_block_utils::ArgvPlan;
use gizza_ai_video_audio_bitrate_set_core::plan;
use wasm_bindgen::prelude::*;

/// `bitrate` is one of the preset kbps strings (`64`…`320`); an empty value lets
/// core apply the 128 kbps default. Returns `{ argv, out_name }` or throws a JS
/// error string.
#[wasm_bindgen]
pub fn build_argv(bitrate: &str, in_name: &str) -> Result<JsValue, JsValue> {
    let (argv, out_name) = plan(in_name, bitrate).map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
