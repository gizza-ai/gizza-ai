//! Browser-facing wasm-bindgen wrapper for /tools/audio-resampler/ (ffmpeg page).
//! Builds the ffmpeg argv (pure, shared with the chat block via core); the JS
//! page driver runs it through the browser ffmpeg bridge.
//!
//! Page field order (meta.toml) MUST match this param order: `rate`, then
//! `format`, then the file (`in_name`). `tool.js` calls
//! `build_argv(...fieldArgs, inName)`.

use wasm_bindgen::prelude::*;

use gizza_ai_audio_resampler_core::{plan_resample, DEFAULT_FORMAT};
use gizza_ai_block_utils::ArgvPlan;

/// `rate` is the target sample rate in Hz (required — 0/empty surfaces as a JS
/// error string via core's range check); `format` is `wav|flac|mp3|ogg|m4a`
/// (empty defaults to wav). Returns `{ argv, out_name }` or throws.
#[wasm_bindgen]
pub fn build_argv(rate: f64, format: &str, in_name: &str) -> Result<JsValue, JsValue> {
    // A blank/negative rate rounds to 0, which core rejects with a clear range
    // message; a fractional rate is rounded to the nearest Hz.
    let rate = if rate.is_finite() && rate > 0.0 {
        rate.round() as u32
    } else {
        0
    };
    let fmt = if format.trim().is_empty() {
        DEFAULT_FORMAT
    } else {
        format
    };
    let (argv, out_name) = plan_resample(in_name, rate, fmt).map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
