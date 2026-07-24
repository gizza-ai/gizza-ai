//! Browser-facing wasm-bindgen wrapper for /tools/audio-bleep-censor/ (ffmpeg page).
//! Builds the ffmpeg argv (pure, shared with the chat block via core); the JS
//! page driver runs it through the browser ffmpeg bridge.
//!
//! Page field order (meta.toml) MUST match this param order: `regions`, `mode`,
//! `tone_hz`, `format`, then the file (`in_name`). `tool.js` calls
//! `build_argv(...fieldArgs, inName)`.

use wasm_bindgen::prelude::*;

use gizza_ai_audio_bleep_censor_core::plan;
use gizza_ai_block_utils::ArgvPlan;

/// `regions` is a comma-separated `start-end` list (seconds or mm:ss/hh:mm:ss),
/// `mode` is `bleep|mute|duck`, `tone_hz` the bleep frequency (100-8000; the
/// page prefills 1000 and ignores it unless mode is bleep), `format` is
/// `mp3|wav|ogg|flac|m4a`. Returns `{ argv, out_name }` or throws an error string.
#[wasm_bindgen]
pub fn build_argv(
    regions: &str,
    mode: &str,
    tone_hz: f64,
    format: &str,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    let (argv, out_name) =
        plan(in_name, regions, mode, tone_hz, format).map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
