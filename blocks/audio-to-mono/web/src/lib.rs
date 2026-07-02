//! Browser-facing wasm-bindgen wrapper for /tools/audio-to-mono/ (ffmpeg
//! page). Builds the ffmpeg argv (pure, shared with the chat block via core);
//! the JS page driver runs it through the browser ffmpeg bridge.
//!
//! Page field order (meta.toml) MUST match this param order: `channel`, then
//! `format`, then the file (`in_name`). `tool.js` calls
//! `build_argv(...fieldArgs, inName)`.

use wasm_bindgen::prelude::*;

use gizza_ai_audio_to_mono_core::plan_to_mono;
use gizza_ai_block_utils::ArgvPlan;

/// `channel` is `mix|left|right` (empty → mix); `format` is
/// `mp3|wav|ogg|flac|m4a` (empty → mp3). Returns `{ argv, out_name }` or
/// throws a JS error string.
#[wasm_bindgen]
pub fn build_argv(channel: &str, format: &str, in_name: &str) -> Result<JsValue, JsValue> {
    let (argv, out_name) =
        plan_to_mono(in_name, channel, format).map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
