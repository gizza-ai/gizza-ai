//! Browser-facing wasm-bindgen wrapper for /tools/audio-fade/ (ffmpeg page).
//! Builds the ffmpeg argv (pure, shared with the chat block via core); the JS
//! page driver runs it through the browser ffmpeg bridge.
//!
//! Page field order (meta.toml) MUST match this param order: `fade_in`, then
//! `fade_out`, then `format`, then the file (`in_name`). `tool.js` calls
//! `build_argv(...fieldArgs, inName)`.

use wasm_bindgen::prelude::*;

use gizza_ai_audio_fade_core::plan_fade;
use gizza_ai_block_utils::ArgvPlan;

/// `fade_in`/`fade_out` are lengths in seconds (0-30). The page prefills both
/// fields with the descriptor default (3); a CLEARED field arrives as 0 and
/// skips that side, so clearing both throws the guiding no-op error. `format`
/// is `mp3|wav|ogg|flac|m4a` (empty defaults to mp3). Returns
/// `{ argv, out_name }` or throws an error string.
#[wasm_bindgen]
pub fn build_argv(
    fade_in: f64,
    fade_out: f64,
    format: &str,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    let (argv, out_name) =
        plan_fade(in_name, fade_in, fade_out, format).map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
