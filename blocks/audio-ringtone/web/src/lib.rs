//! Browser-facing wasm-bindgen wrapper for /tools/audio-ringtone/ (ffmpeg page).
//! Builds the ffmpeg argv (pure, shared with the chat block via core); the JS
//! page driver runs it through the browser ffmpeg bridge.
//!
//! Page field order (meta.toml) MUST match this param order: `start`, `end`,
//! `fade_in`, `fade_out`, `normalize`, `format`, then the file (`in_name`).
//! `tool.js` calls `build_argv(...fieldArgs, inName)`.

use wasm_bindgen::prelude::*;

use gizza_ai_audio_ringtone_core::plan_ringtone;
use gizza_ai_block_utils::ArgvPlan;

/// `start`/`end` are the selection bounds in seconds (empty page fields arrive
/// as `0.0` — start=0 means "from the beginning", end=0/empty means
/// "start + 30 s", the standard ringtone length; the slice is capped at 40 s).
/// `fade_in`/`fade_out` are seconds (0-5). `normalize` is the checkbox value
/// string (positive truthy). `format` is `m4r|mp3` (empty → m4r). Returns
/// `{ argv, out_name }` or throws.
#[wasm_bindgen]
pub fn build_argv(
    start: f64,
    end: f64,
    fade_in: f64,
    fade_out: f64,
    normalize: &str,
    format: &str,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    let normalize_on = matches!(normalize, "true" | "1" | "on" | "yes");
    let (argv, out_name) = plan_ringtone(in_name, start, end, fade_in, fade_out, normalize_on, format)
        .map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
