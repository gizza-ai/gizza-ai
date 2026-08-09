//! Browser-facing wasm-bindgen wrapper for /tools/audio-reverse/ (ffmpeg page).
//! Builds the ffmpeg argv (pure, shared with the chat block via core); the JS
//! page driver runs it through the browser ffmpeg bridge.
//!
//! Page field order (meta.toml) MUST match this param order: `mode`, then
//! `format`, then the file (`in_name`). `tool.js` calls
//! `build_argv(...fieldArgs, inName)`.

use wasm_bindgen::prelude::*;

use gizza_ai_audio_reverse_core::plan;
use gizza_ai_block_utils::ArgvPlan;

/// `mode` is `reverse|forward-reverse|reverse-forward` (empty → reverse) and
/// `format` is `mp3|wav|ogg|flac|m4a` (empty → mp3); both are `<select>`s on the
/// page, so the empty fallbacks only matter for hand-built deep links. Returns
/// `{ argv, out_name }` or throws a JS error string.
#[wasm_bindgen]
pub fn build_argv(mode: &str, format: &str, in_name: &str) -> Result<JsValue, JsValue> {
    let (argv, out_name) = plan(in_name, mode, format).map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
