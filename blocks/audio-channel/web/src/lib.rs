//! Browser-facing wasm-bindgen wrapper for /tools/audio-channel/ (ffmpeg
//! page). Builds the ffmpeg argv (pure, shared with the chat block via core);
//! the JS page driver runs it through the browser ffmpeg bridge.
//!
//! Page field order (meta.toml) MUST match this param order: `operation`, then
//! `format`, then the file (`in_name`). `tool.js` calls
//! `build_argv(...fieldArgs, inName)`.

use wasm_bindgen::prelude::*;

use gizza_ai_audio_channel_core::plan_channels;
use gizza_ai_block_utils::ArgvPlan;

/// `operation` is `swap|mono|stereo|left|right` (empty → swap); `format` is
/// `mp3|wav|ogg|flac|m4a` (empty → mp3). Returns `{ argv, out_name }` or
/// throws a JS error string.
#[wasm_bindgen]
pub fn build_argv(operation: &str, format: &str, in_name: &str) -> Result<JsValue, JsValue> {
    let (argv, out_name) =
        plan_channels(in_name, operation, format).map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
