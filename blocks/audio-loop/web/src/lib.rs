//! Browser-facing wasm-bindgen wrapper for /tools/audio-loop/ (ffmpeg page).
//! Builds the ffmpeg argv (pure, shared with the chat block via core); the JS
//! page driver runs it through the browser ffmpeg bridge.
//!
//! Page field order (meta.toml) MUST match this param order: `duration`, then
//! `count`, then `format`, then the file (`in_name`). `tool.js` calls
//! `build_argv(...fieldArgs, inName)`.

use wasm_bindgen::prelude::*;

use gizza_ai_audio_loop_core::plan_loop;
use gizza_ai_block_utils::ArgvPlan;

/// `duration` is the target output length in seconds (page prefills the
/// 30 s default; cleared = 0 = use `count`); `count` is total plays 2-100
/// (empty = 0); `format` is `mp3|wav|ogg|flac|m4a` (empty defaults to mp3).
/// Returns `{ argv, out_name }` or throws an error string.
#[wasm_bindgen]
pub fn build_argv(
    duration: f64,
    count: f64,
    format: &str,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    // Negative/NaN counts saturate to 0 in the cast and are rejected by the plan.
    let count = count.round() as u32;
    let (argv, out_name) =
        plan_loop(in_name, duration, count, format).map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
