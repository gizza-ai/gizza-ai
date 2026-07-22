//! Browser-facing wasm-bindgen wrapper for /tools/audio-edge-silence-trimmer/
//! (ffmpeg page). Builds the ffmpeg argv (pure, shared with the chat block via
//! core); the JS page driver runs it through the browser ffmpeg bridge.
//!
//! Page field order (meta.toml) MUST match this param order: `threshold_db`,
//! then `pad`, then `format`, then the file (`in_name`). `tool.js` calls
//! `build_argv(...fieldArgs, inName)`.

use wasm_bindgen::prelude::*;

use gizza_ai_audio_edge_silence_trimmer_core::{plan_edge_trim, DEFAULT_PAD_S, DEFAULT_THRESHOLD_DB};
use gizza_ai_block_utils::ArgvPlan;

/// Empty page number fields arrive as `0.0`. A 0 dB threshold would treat
/// everything as silence, so `threshold_db == 0` means "use the default"
/// (-50 dB). `pad == 0` is a legitimate value (hard cut), so it is passed
/// through as-is. `format` is `mp3|wav|ogg|flac|m4a` (empty → mp3). Returns
/// `{ argv, out_name }` or throws a JS error string.
#[wasm_bindgen]
pub fn build_argv(
    threshold_db: f64,
    pad: f64,
    format: &str,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    let t = if threshold_db == 0.0 {
        DEFAULT_THRESHOLD_DB
    } else {
        threshold_db
    };
    // pad == 0.0 is a valid hard cut; only substitute the default when the field
    // is left blank AND threshold was also blank (page never sends a bare pad).
    let p = if pad == 0.0 && threshold_db == 0.0 {
        DEFAULT_PAD_S
    } else {
        pad
    };
    let (argv, out_name) =
        plan_edge_trim(in_name, t, p, format).map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
