//! Browser-facing wasm-bindgen wrapper for /tools/audio-declick/ (ffmpeg page).
//! Builds the ffmpeg argv (pure, shared with the chat block via core); returns
//! the shared block_utils::ArgvPlan so the page driver gets { argv, out_name }.
//!
//! Page field order (meta.toml) MUST match this param order: `strength`,
//! `window_ms`, `burst`, `method`, `declip`, `format`, then the file
//! (`in_name`). `tool.js` calls `build_argv(...fieldArgs, inName)`. Checkboxes
//! arrive as `"true"`/`"false"`; numeric fields arrive coerced to numbers, and
//! an emptied numeric box arrives as `0`.

use wasm_bindgen::prelude::*;

use gizza_ai_audio_declick_core::{plan, DEFAULT_STRENGTH, DEFAULT_WINDOW_MS};
use gizza_ai_block_utils::ArgvPlan;

/// `strength` is the 1–100 detection intensity and `window_ms` the 10–100 ms
/// repair window — both are rendered pre-filled with their schema default, and
/// an emptied box arrives as `0.0`, which is below their minimum, so it is read
/// as "use the default" rather than erroring. `burst` is 0–10 where **0 is a
/// real value** (fusion off), so it is passed straight through — clearing that
/// box therefore reads as 0/off. `method` is `add|save` (empty → add), `declip`
/// arrives as `"true"`/`"false"` from the checkbox, and `format` is
/// `mp3|wav|ogg|flac|m4a` (empty → mp3). Returns `{ argv, out_name }` or throws.
#[wasm_bindgen]
pub fn build_argv(
    strength: f64,
    window_ms: f64,
    burst: f64,
    method: &str,
    declip: &str,
    format: &str,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    let s = if strength == 0.0 {
        DEFAULT_STRENGTH
    } else {
        strength
    };
    let w = if window_ms == 0.0 {
        DEFAULT_WINDOW_MS
    } else {
        window_ms
    };
    let b = if burst.is_finite() {
        burst.round() as i64
    } else {
        -1 // let core reject it with its range message
    };
    let dc = matches!(declip.trim(), "true" | "1" | "on" | "yes");
    let (argv, out_name) =
        plan(in_name, s, w, b, method, dc, format).map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
