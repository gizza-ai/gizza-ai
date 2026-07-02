//! Browser-facing wasm-bindgen wrapper for /tools/audio-convert/ (ffmpeg page).
//! Builds the ffmpeg argv (pure, shared with the chat block via core); the JS
//! page driver runs it through the browser ffmpeg bridge.
//!
//! Page field order (meta.toml) MUST match this param order: `format`, then
//! `bitrate`, then the file (`in_name`). `tool.js` calls
//! `build_argv(...fieldArgs, inName)`.

use wasm_bindgen::prelude::*;

use gizza_ai_audio_convert_core::{plan_convert, DEFAULT_BITRATE};
use gizza_ai_block_utils::ArgvPlan;

/// `format` is `mp3|wav|ogg|flac|m4a` (required — an empty value surfaces as a
/// JS error string); `bitrate` is the lossy kbps (0/empty defaults to 192,
/// ignored for wav/flac). Returns `{ argv, out_name }` or throws.
#[wasm_bindgen]
pub fn build_argv(format: &str, bitrate: f64, in_name: &str) -> Result<JsValue, JsValue> {
    let kbps = if bitrate > 0.0 {
        bitrate.round().clamp(32.0, 320.0) as u32
    } else {
        DEFAULT_BITRATE
    };
    let (argv, out_name) = plan_convert(in_name, format, kbps).map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
