//! Browser-facing wasm-bindgen wrapper for /tools/audio-fit-to-length/ (ffmpeg page).
//! Builds the ffmpeg argv (pure, shared with the chat block via core); the JS
//! page driver runs it through the browser ffmpeg bridge.
//!
//! Page field order (meta.toml) MUST match this param order: `duration`, then
//! `pad`, then `format`, then the file (`in_name`). `tool.js` calls
//! `build_argv(...fieldArgs, inName)` — numeric fields arrive coerced to Number,
//! string fields as strings.

use wasm_bindgen::prelude::*;

use gizza_ai_audio_fit_to_length_core::plan_fit;
use gizza_ai_block_utils::ArgvPlan;

/// `duration` is the exact target output length in seconds (page prefills the
/// 30 s default); `pad` is `end|start` (empty defaults to end); `format` is
/// `mp3|wav|ogg|flac|m4a` (empty defaults to mp3). Returns `{ argv, out_name }`
/// or throws an error string.
#[wasm_bindgen]
pub fn build_argv(
    duration: f64,
    pad: &str,
    format: &str,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    let (argv, out_name) =
        plan_fit(in_name, duration, pad, format).map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
