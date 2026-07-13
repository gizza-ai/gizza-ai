//! Browser-facing wasm-bindgen wrapper for /tools/voice-note-converter/ (ffmpeg page).
//! Builds the ffmpeg argv (pure, shared with the chat block via core); the JS
//! page driver runs it through the browser ffmpeg bridge.
//!
//! Page field order (meta.toml) MUST match this param order: `format`, then
//! `bitrate`, then `mono`, then the file (`in_name`). `tool.js` calls
//! `build_argv(...fieldArgs, inName)`.

use wasm_bindgen::prelude::*;

use gizza_ai_block_utils::ArgvPlan;
use gizza_ai_voice_note_converter_core::plan_convert;

fn truthy(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "" | "true" | "1" | "on" | "yes"
    )
}

/// `format` is `opus|mp3|wav` (required); `bitrate` is the lossy kbps (0/empty
/// picks the per-format default: opus 32, mp3 128; ignored for wav); `mono`
/// defaults on in the page checkbox and selects Opus' voice-tuned mode.
#[wasm_bindgen]
pub fn build_argv(format: &str, bitrate: f64, mono: &str, in_name: &str) -> Result<JsValue, JsValue> {
    let kbps = if bitrate > 0.0 {
        Some(bitrate.round().clamp(6.0, 320.0) as u32)
    } else {
        None
    };
    let (argv, out_name) = plan_convert(in_name, format, kbps, truthy(mono))
        .map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
