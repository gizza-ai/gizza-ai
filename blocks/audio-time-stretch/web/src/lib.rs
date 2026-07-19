//! Browser-facing wasm-bindgen wrapper for /tools/audio-time-stretch/ (ffmpeg
//! page). Builds the ffmpeg argv (pure, shared with the chat block via core);
//! the JS page driver runs it through the browser ffmpeg bridge.
//!
//! Page field order (meta.toml) MUST match this param order: `factor`,
//! `format`, then the file (`in_name`). `tool.js` calls
//! `build_argv(...fieldArgs, inName)`.

use wasm_bindgen::prelude::*;

use gizza_ai_audio_time_stretch_core::plan_time_stretch;
use gizza_ai_block_utils::ArgvPlan;

/// `factor` is the playback-speed multiplier (an empty page field arrives as
/// `0.0`, which core rejects with the guiding error); `format` is
/// `mp3|wav|ogg|flac|m4a` (empty → mp3). Returns `{ argv, out_name }` or throws.
#[wasm_bindgen]
pub fn build_argv(factor: f64, format: &str, in_name: &str) -> Result<JsValue, JsValue> {
    let (argv, out_name) =
        plan_time_stretch(in_name, factor, format).map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
