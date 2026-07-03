//! Browser-facing wasm-bindgen wrapper for /tools/trim-audio/ (ffmpeg page).
//! Builds the ffmpeg argv (pure, shared with the chat block via core); the JS
//! page driver runs it through the browser ffmpeg bridge.
//!
//! Page field order (meta.toml) MUST match this param order: `start`, `end`,
//! `mode`, `format`, `fade`, then the file (`in_name`). `tool.js` calls
//! `build_argv(...fieldArgs, inName)`.

use wasm_bindgen::prelude::*;

use gizza_ai_block_utils::ArgvPlan;
use gizza_ai_trim_audio_core::plan_trim_audio;

/// `start`/`end` are the selection bounds in seconds (empty page fields arrive
/// as `0.0` — start=0 means "from the beginning", end=0/empty means "to the
/// end of the track"; both at once is the guiding whole-file error). `mode` is
/// `keep|remove` (empty → keep), `format` is `mp3|wav|ogg|flac|m4a` (empty → mp3),
/// and `fade` is the checkbox value string (positive truthy). Returns
/// `{ argv, out_name }` or throws.
#[wasm_bindgen]
pub fn build_argv(
    start: f64,
    end: f64,
    mode: &str,
    format: &str,
    fade: &str,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    let fade_on = matches!(fade, "true" | "1" | "on" | "yes");
    let (argv, out_name) = plan_trim_audio(in_name, start, end, mode, format, fade_on)
        .map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
