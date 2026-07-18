//! Browser-facing wasm-bindgen wrapper for /tools/audio-highpass-filter/ (ffmpeg
//! page). Builds the ffmpeg argv (pure, shared with the chat block via core);
//! the JS page driver runs it through the browser ffmpeg bridge.
//!
//! Page field order (meta.toml) MUST match this param order: `cutoff`,
//! `rolloff`, `format`, then the file (`in_name`). `tool.js` calls
//! `build_argv(...fieldArgs, inName)`.

use wasm_bindgen::prelude::*;

use gizza_ai_audio_highpass_filter_core::{plan, DEFAULT_CUTOFF};
use gizza_ai_block_utils::ArgvPlan;

/// `cutoff` is the corner frequency in Hz (an empty page field arrives as `0.0`,
/// which is below the [10, 2000] range — treat it as "use the default" rather
/// than erroring). `rolloff` is `6|12|24|48` dB/oct (empty → 12). `format` is
/// `mp3|wav|ogg|flac|m4a` (empty → mp3). Returns `{ argv, out_name }` or throws.
#[wasm_bindgen]
pub fn build_argv(
    cutoff: f64,
    rolloff: &str,
    format: &str,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    let c = if cutoff == 0.0 { DEFAULT_CUTOFF } else { cutoff };
    let (argv, out_name) = plan(in_name, c, rolloff, format).map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
