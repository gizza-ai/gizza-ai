//! Browser-facing wasm-bindgen wrapper for /tools/audio-noise-reduce/ (ffmpeg
//! page). Builds the ffmpeg argv (pure, shared with the chat block via core);
//! the JS page driver runs it through the browser ffmpeg bridge.
//!
//! Page field order (meta.toml) MUST match this param order: `strength`,
//! `method`, `remove_hum`, `format`, then the file (`in_name`). `tool.js` calls
//! `build_argv(...fieldArgs, inName)`. Checkboxes arrive as `"true"`/`"false"`.

use wasm_bindgen::prelude::*;

use gizza_ai_audio_noise_reduce_core::{plan, DEFAULT_STRENGTH};
use gizza_ai_block_utils::ArgvPlan;

/// `strength` is the 1–100 denoise intensity (an empty page field arrives as
/// `0.0`, which is outside the [1, 100] range — treat it as "use the default"
/// rather than erroring). `method` is `afftdn|anlmdn` (empty → afftdn).
/// `remove_hum` arrives as `"true"`/`"false"` from the checkbox. `format` is
/// `mp3|wav|ogg|flac|m4a` (empty → mp3). Returns `{ argv, out_name }` or throws.
#[wasm_bindgen]
pub fn build_argv(
    strength: f64,
    method: &str,
    remove_hum: &str,
    format: &str,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    let s = if strength == 0.0 {
        DEFAULT_STRENGTH
    } else {
        strength
    };
    let hum = matches!(remove_hum.trim(), "true" | "1" | "on" | "yes");
    let (argv, out_name) =
        plan(in_name, s, method, hum, format).map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
