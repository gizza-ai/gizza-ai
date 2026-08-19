//! Browser-facing wasm-bindgen wrapper for /tools/audio-limiter/ (ffmpeg page).
//! Builds the ffmpeg argv (pure, shared with the chat block via core); the JS
//! page driver runs it through the browser ffmpeg bridge.
//!
//! Page field order (meta.toml) MUST match this param order: `ceiling`, `gain`,
//! `attack`, `release`, `smooth_release`, `auto_level`, then `format`, then the
//! file (`in_name`). `tool.js` calls `build_argv(...fieldArgs, inName)`.

use wasm_bindgen::prelude::*;

use gizza_ai_audio_limiter_core::{
    plan_limit, DEFAULT_ATTACK_MS, DEFAULT_CEILING_DB, DEFAULT_RELEASE_MS,
};
use gizza_ai_block_utils::ArgvPlan;

/// The four numeric controls, the two checkboxes, and `format`
/// (`mp3|wav|ogg|flac|m4a`, empty → mp3). An empty page number field arrives
/// here as `0.0`; for `attack` and `release` zero is below the valid minimum so
/// it unambiguously means "use the default". For `ceiling`, 0 dBFS is the top of
/// the valid range but also what a blank field sends, so a blank/zero ceiling
/// becomes the −1 dB default (a 0 dB ceiling with no gain is the rejected no-op
/// anyway — drive it with `gain` if you really want to limit at full scale from
/// the page). `gain` needs no fallback: its default is 0, exactly what a blank
/// field sends. Checkboxes arrive as `"true"`/`"false"` — parse positive-truthy.
/// Returns `{ argv, out_name }` or throws a JS error string.
#[wasm_bindgen]
pub fn build_argv(
    ceiling: f64,
    gain: f64,
    attack: f64,
    release: f64,
    smooth_release: &str,
    auto_level: &str,
    format: &str,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    let ceiling = if ceiling == 0.0 {
        DEFAULT_CEILING_DB
    } else {
        ceiling
    };
    let attack = if attack == 0.0 {
        DEFAULT_ATTACK_MS
    } else {
        attack
    };
    let release = if release == 0.0 {
        DEFAULT_RELEASE_MS
    } else {
        release
    };
    let truthy = |v: &str| matches!(v, "true" | "1" | "on" | "yes");
    let (argv, out_name) = plan_limit(
        in_name,
        ceiling,
        gain,
        attack,
        release,
        truthy(smooth_release),
        truthy(auto_level),
        format,
    )
    .map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
