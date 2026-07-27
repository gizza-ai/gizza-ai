//! Browser-facing wasm-bindgen wrapper for /tools/audio-filter/ (ffmpeg page).
//! Builds the ffmpeg argv (pure, shared with the chat block via core); the JS
//! page driver runs it through the browser ffmpeg bridge.
//!
//! Page field order (meta.toml) MUST match this param order: `type`,
//! `frequency`, `width`, `format`, then the file (`in_name`). `tool.js` calls
//! `build_argv(...fieldArgs, inName)`.

use wasm_bindgen::prelude::*;

use gizza_ai_audio_filter_core::{plan, DEFAULT_FREQ, DEFAULT_WIDTH};
use gizza_ai_block_utils::ArgvPlan;

/// `filter_type` is `lowpass|highpass|bandpass|notch` (empty → lowpass).
/// `frequency` is the corner/centre in Hz (an empty page field arrives as `0.0`,
/// which is below the [20, 20000] range — treat it as "use the default" rather
/// than erroring). `width` is the band width in Hz for band-pass/notch (empty
/// `0.0` → default 200; ignored for low-/high-pass). `format` is
/// `mp3|wav|ogg|flac|m4a` (empty → mp3). Returns `{ argv, out_name }` or throws.
#[wasm_bindgen]
pub fn build_argv(
    filter_type: &str,
    frequency: f64,
    width: f64,
    format: &str,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    let f = if frequency == 0.0 { DEFAULT_FREQ } else { frequency };
    let w = if width == 0.0 { DEFAULT_WIDTH } else { width };
    let (argv, out_name) =
        plan(in_name, filter_type, f, w, format).map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
