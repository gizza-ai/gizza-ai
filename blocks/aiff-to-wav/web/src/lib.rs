//! Browser-facing wasm-bindgen wrapper for /tools/aiff-to-wav/ (ffmpeg page).
//! Builds the ffmpeg argv (pure, shared with the chat block via core); the JS
//! page driver runs it through the browser ffmpeg bridge.
//!
//! Page field order (meta.toml) MUST match this param order: `bit_depth`,
//! `sample_rate`, `channels`, `keep_metadata`, then the file (`in_name`).
//! `tool.js` calls `build_argv(...fieldArgs, inName)`.

use wasm_bindgen::prelude::*;

use gizza_ai_aiff_to_wav_core::{
    plan, DEFAULT_BIT_DEPTH, DEFAULT_CHANNELS, DEFAULT_KEEP_METADATA, DEFAULT_SAMPLE_RATE,
};
use gizza_ai_block_utils::ArgvPlan;

/// A blank select/deep-link value means "use the tool default" rather than an
/// error — the page should still produce a valid WAV from an empty control.
fn or_default<'a>(value: &'a str, fallback: &'a str) -> &'a str {
    if value.is_empty() {
        fallback
    } else {
        value
    }
}

/// The checkbox marshals as `"true"`/`"false"` via `readField`; a deep link can
/// also send `1`/`on`/`yes`. Parse positive-truthy, and treat anything blank as
/// the descriptor default (checked).
fn checkbox(value: &str, fallback: bool) -> bool {
    if value.is_empty() {
        return fallback;
    }
    matches!(
        value.to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

/// `bit_depth` / `sample_rate` / `channels` are the enum values advertised by
/// the descriptor (blank = default); `keep_metadata` is the checkbox state.
/// `in_name` is the uploaded file's name. Returns `{ argv, out_name }` or throws.
#[wasm_bindgen]
pub fn build_argv(
    bit_depth: &str,
    sample_rate: &str,
    channels: &str,
    keep_metadata: &str,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    let (argv, out_name) = plan(
        in_name,
        or_default(bit_depth, DEFAULT_BIT_DEPTH),
        or_default(sample_rate, DEFAULT_SAMPLE_RATE),
        or_default(channels, DEFAULT_CHANNELS),
        checkbox(keep_metadata, DEFAULT_KEEP_METADATA),
    )
    .map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
