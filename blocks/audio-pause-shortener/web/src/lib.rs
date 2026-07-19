//! Browser-facing wasm-bindgen wrapper for /tools/audio-pause-shortener/
//! (ffmpeg page). Builds the ffmpeg argv (pure, shared with the chat block via
//! core); the JS page driver runs it through the browser ffmpeg bridge.
//!
//! Page field order (meta.toml) MUST match this param order: `threshold_db`,
//! `max_pause`, `target_pause`, `format`, then the file (`in_name`). `tool.js`
//! calls `build_argv(...fieldArgs, inName)`.

use wasm_bindgen::prelude::*;

use gizza_ai_audio_pause_shortener_core::{
    plan_pause_shorten, DEFAULT_MAX_PAUSE_S, DEFAULT_TARGET_PAUSE_S, DEFAULT_THRESHOLD_DB,
};
use gizza_ai_block_utils::ArgvPlan;

/// Empty page number fields arrive as `0.0`. threshold 0 dB and a 0 s max_pause
/// are not useful defaults, so those zeros mean "use the default" (-30 dB /
/// 1.5 s). `target_pause` legitimately accepts values down to 0 but the page
/// exposes a sensible non-zero default, so a `0.0` from an empty field is also
/// treated as "use the default" (0.5 s). `format` is `mp3|wav|ogg|flac|m4a`
/// (empty → mp3). Returns `{ argv, out_name }` or throws a JS error string.
#[wasm_bindgen]
pub fn build_argv(
    threshold_db: f64,
    max_pause: f64,
    target_pause: f64,
    format: &str,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    let t = if threshold_db == 0.0 {
        DEFAULT_THRESHOLD_DB
    } else {
        threshold_db
    };
    let m = if max_pause == 0.0 {
        DEFAULT_MAX_PAUSE_S
    } else {
        max_pause
    };
    let k = if target_pause == 0.0 {
        DEFAULT_TARGET_PAUSE_S
    } else {
        target_pause
    };
    let (argv, out_name) =
        plan_pause_shorten(in_name, t, m, k, format).map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
