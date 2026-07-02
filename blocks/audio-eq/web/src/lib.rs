//! Browser-facing wasm-bindgen wrapper for /tools/audio-eq/ (ffmpeg page).
//! Builds the ffmpeg argv (pure, shared with the chat block via core); the JS
//! page driver runs it through the browser ffmpeg bridge.
//!
//! Page field order (meta.toml) MUST match this param order: `bass`, `mid`,
//! `treble`, then `format`, then the file (`in_name`). `tool.js` calls
//! `build_argv(...fieldArgs, inName)`.

use wasm_bindgen::prelude::*;

use gizza_ai_audio_eq_core::plan_eq;
use gizza_ai_block_utils::ArgvPlan;

/// `bass`/`mid`/`treble` are gains in dB (-20..20; empty fields arrive as 0 =
/// band unchanged; all three at 0 throws the guiding no-op error); `format` is
/// `mp3|wav|ogg|flac|m4a` (empty defaults to mp3). Returns `{ argv, out_name }`
/// or throws an error string.
#[wasm_bindgen]
pub fn build_argv(
    bass: f64,
    mid: f64,
    treble: f64,
    format: &str,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    let (argv, out_name) =
        plan_eq(in_name, bass, mid, treble, format).map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
