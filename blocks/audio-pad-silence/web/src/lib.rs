//! Browser-facing wasm-bindgen wrapper for /tools/audio-pad-silence/ (ffmpeg page).
//! Builds the ffmpeg argv (pure, shared with the chat block via core); the JS
//! page driver runs it through the browser ffmpeg bridge.
//!
//! Page field order (meta.toml) MUST match this param order: `start`, then
//! `end`, then `format`, then the file (`in_name`). `tool.js` calls
//! `build_argv(...fieldArgs, inName)` — numeric fields arrive coerced to Number,
//! string fields as strings.

use wasm_bindgen::prelude::*;

use gizza_ai_audio_pad_silence_core::plan_pad;
use gizza_ai_block_utils::ArgvPlan;

/// `start`/`end` are seconds of silence to add before/after the clip (the page
/// prefills 2 and 0); at least one must be greater than 0. `format` is
/// `mp3|wav|ogg|flac|m4a` (empty defaults to mp3). Returns `{ argv, out_name }`
/// or throws an error string.
#[wasm_bindgen]
pub fn build_argv(start: f64, end: f64, format: &str, in_name: &str) -> Result<JsValue, JsValue> {
    let (argv, out_name) =
        plan_pad(in_name, start, end, format).map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
