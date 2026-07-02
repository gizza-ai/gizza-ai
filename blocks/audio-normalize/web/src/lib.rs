//! Browser-facing wasm-bindgen wrapper for /tools/audio-normalize/ (ffmpeg
//! page). Builds the ffmpeg argv (pure, shared with the chat block via core);
//! the JS page driver runs it through the browser ffmpeg bridge.
//!
//! Page field order (meta.toml) MUST match this param order: `lufs`, then
//! `format`, then the file (`in_name`). `tool.js` calls
//! `build_argv(...fieldArgs, inName)`.

use wasm_bindgen::prelude::*;

use gizza_ai_audio_normalize_core::{plan_normalize, DEFAULT_LUFS};
use gizza_ai_block_utils::ArgvPlan;

/// `lufs` is the target integrated loudness (an empty page field arrives as
/// `0.0`, which is outside loudnorm's [-70, -5] range — treat it as "use the
/// -14 default" rather than erroring). `format` is `mp3|wav|ogg|flac|m4a`
/// (empty → mp3). Returns `{ argv, out_name }` or throws a JS error string.
#[wasm_bindgen]
pub fn build_argv(lufs: f64, format: &str, in_name: &str) -> Result<JsValue, JsValue> {
    let target = if lufs == 0.0 { DEFAULT_LUFS } else { lufs };
    let (argv, out_name) =
        plan_normalize(in_name, target, format).map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
