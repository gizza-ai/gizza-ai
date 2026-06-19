//! Browser-facing wasm-bindgen wrapper for /tools/video-transcode/ (ffmpeg page).
//! Builds the ffmpeg argv (pure, shared with the chat block via core); the JS
//! page driver runs it through the browser ffmpeg bridge.
//!
//! Page field order (meta.toml) MUST match this param order: `format`, then
//! `quality`, then the file (`in_name`). `tool.js` calls
//! `build_argv(...fieldArgs, inName)`.

use wasm_bindgen::prelude::*;

use gizza_ai_block_utils::ArgvPlan;
use gizza_ai_video_transcode_core::{plan_transcode, DEFAULT_QUALITY};

/// `format` is one of `mp4|webm`; `quality` is 1-100 (0/empty defaults to 75).
/// Returns `{ argv: string[], out_name }` (output uses the target format's
/// extension) or throws a JS error string.
#[wasm_bindgen]
pub fn build_argv(format: &str, quality: f64, in_name: &str) -> Result<JsValue, JsValue> {
    let q = if quality > 0.0 {
        quality.round().clamp(1.0, 100.0) as u8
    } else {
        DEFAULT_QUALITY
    };
    let (argv, out_name) = plan_transcode(format, q, in_name).map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
