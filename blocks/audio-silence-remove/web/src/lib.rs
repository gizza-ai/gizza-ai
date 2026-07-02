//! Browser-facing wasm-bindgen wrapper for /tools/audio-silence-remove/
//! (ffmpeg page). Builds the ffmpeg argv (pure, shared with the chat block via
//! core); the JS page driver runs it through the browser ffmpeg bridge.
//!
//! Page field order (meta.toml) MUST match this param order: `threshold_db`,
//! then `min_silence`, then `format`, then the file (`in_name`). `tool.js`
//! calls `build_argv(...fieldArgs, inName)`.

use wasm_bindgen::prelude::*;

use gizza_ai_audio_silence_remove_core::{
    plan_silence_remove, DEFAULT_MIN_SILENCE_S, DEFAULT_THRESHOLD_DB,
};
use gizza_ai_block_utils::ArgvPlan;

/// Empty page number fields arrive as `0.0`: threshold 0 dB would treat
/// everything as silence and a 0 s gap is invalid, so both zeros mean "use the
/// default" (-30 dB / 0.5 s). `format` is `mp3|wav|ogg|flac|m4a` (empty →
/// mp3). Returns `{ argv, out_name }` or throws a JS error string.
#[wasm_bindgen]
pub fn build_argv(
    threshold_db: f64,
    min_silence: f64,
    format: &str,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    let t = if threshold_db == 0.0 {
        DEFAULT_THRESHOLD_DB
    } else {
        threshold_db
    };
    let d = if min_silence == 0.0 {
        DEFAULT_MIN_SILENCE_S
    } else {
        min_silence
    };
    let (argv, out_name) =
        plan_silence_remove(in_name, t, d, format).map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
