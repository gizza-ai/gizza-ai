//! Browser-facing wasm-bindgen wrapper for /tools/audio-noise-gate/ (ffmpeg
//! page). Builds the ffmpeg argv (pure, shared with the chat block via core);
//! the JS page driver runs it through the browser ffmpeg bridge.
//!
//! Page field order (meta.toml) MUST match this param order: `threshold`,
//! `reduction`, `ratio`, `attack`, `release`, `detection`, then `format`, then
//! the file (`in_name`). `tool.js` calls `build_argv(...fieldArgs, inName)`.

use wasm_bindgen::prelude::*;

use gizza_ai_audio_noise_gate_core::{
    plan_gate, DEFAULT_ATTACK_MS, DEFAULT_RATIO, DEFAULT_REDUCTION_DB, DEFAULT_RELEASE_MS,
    DEFAULT_THRESHOLD_DB,
};
use gizza_ai_block_utils::ArgvPlan;

/// The five numeric gate controls plus `detection` (`rms|peak`, empty → rms) and
/// `format` (`mp3|wav|ogg|flac|m4a`, empty → mp3). An empty page number field
/// arrives here as `0.0`; for `reduction`, `ratio`, `attack` and `release` zero
/// is below the valid minimum (or, for reduction, the no-op the core rejects) so
/// it unambiguously means "use the default". For `threshold`, 0 dB is the top of
/// the valid range but also what a blank field sends, so a blank/zero threshold
/// is treated as the −35 dB default (a 0 dB threshold would gate almost nothing
/// anyway). Returns `{ argv, out_name }` or throws a JS error string.
#[wasm_bindgen]
pub fn build_argv(
    threshold: f64,
    reduction: f64,
    ratio: f64,
    attack: f64,
    release: f64,
    detection: &str,
    format: &str,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    let threshold = if threshold == 0.0 {
        DEFAULT_THRESHOLD_DB
    } else {
        threshold
    };
    let reduction = if reduction == 0.0 {
        DEFAULT_REDUCTION_DB
    } else {
        reduction
    };
    let ratio = if ratio == 0.0 { DEFAULT_RATIO } else { ratio };
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
    let (argv, out_name) = plan_gate(
        in_name, threshold, reduction, ratio, attack, release, detection, format,
    )
    .map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
