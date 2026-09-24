//! Browser-facing wasm-bindgen wrapper for /tools/gif-frame-deduplicator/
//! (ffmpeg page). Builds the ffmpeg argv (pure, shared with the chat block via
//! core); returns the shared `block_utils::ArgvPlan` so the page driver gets
//! `{ argv, out_name }`.
//!
//! Page field order (meta.toml) MUST match this param order: `threshold`, then
//! the file (`in_name`). `tool.js` calls `build_argv(...fieldArgs, inName)`.
use gizza_ai_block_utils::ArgvPlan;
use gizza_ai_gif_frame_deduplicator_core::{plan, DEFAULT_THRESHOLD};
use wasm_bindgen::prelude::*;

/// `threshold` is the 0-100 similarity percentage above which two consecutive
/// frames count as duplicates. An empty page field arrives here as `0.0`, which
/// would mean "treat every frame as a duplicate" — so a non-positive or
/// non-finite value is read as *unset* and falls back to the default (98).
/// Returns `{ argv, out_name }` (always `optimized.gif`) or throws a JS error
/// string (e.g. when the picked file is not a GIF).
#[wasm_bindgen]
pub fn build_argv(threshold: f64, in_name: &str) -> Result<JsValue, JsValue> {
    let threshold = if threshold.is_finite() && threshold > 0.0 {
        threshold
    } else {
        DEFAULT_THRESHOLD
    };
    let (argv, out_name) = plan(in_name, threshold).map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
