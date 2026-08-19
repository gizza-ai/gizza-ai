//! Browser-facing wasm-bindgen wrapper for /tools/video-set-keyframe-interval/
//! (ffmpeg page). Builds the ffmpeg argv (pure, shared with the chat block via
//! core); the JS page driver runs it through the browser ffmpeg bridge.
//!
//! Page field order (meta.toml) MUST match this param order: `interval`, `unit`,
//! `scene_cut`, `closed_gop`, `quality`, `preset`, then the file (`in_name`).
//! `tool.js` calls `build_argv(...fieldArgs, inName)`.

use gizza_ai_block_utils::ArgvPlan;
use gizza_ai_video_set_keyframe_interval_core::{
    plan, DEFAULT_INTERVAL, DEFAULT_PRESET, DEFAULT_QUALITY,
};
use wasm_bindgen::prelude::*;

/// Checkboxes arrive from the page driver as `"true"`/`"false"` strings — parse
/// positive-truthy so any of the usual spellings works.
fn truthy(v: &str) -> bool {
    matches!(v.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

/// `interval` is in the unit given by `unit` (`seconds|frames`; empty = seconds);
/// `0`/empty interval falls back to 2. `quality` is libx264 CRF 1-51; an empty
/// field marshals as `0`, which falls back to 23 (CRF 0 is not an accepted value
/// on any surface — see the core's `MIN_CRF`). Returns
/// `{ argv, out_name: "out.mp4" }` or throws a JS error string.
#[wasm_bindgen]
pub fn build_argv(
    interval: f64,
    unit: &str,
    scene_cut: &str,
    closed_gop: &str,
    quality: f64,
    preset: &str,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    let interval = if interval > 0.0 {
        interval
    } else {
        DEFAULT_INTERVAL
    };
    let unit = if unit.trim().is_empty() {
        "seconds"
    } else {
        unit.trim()
    };
    // An empty number field marshals as 0.0 — treat that as "unset". CRF 0
    // (true lossless) is not accepted on any surface, so nothing is lost.
    let quality = if quality.is_finite() && quality >= 1.0 {
        quality.round() as u32
    } else {
        DEFAULT_QUALITY
    };
    let preset = if preset.trim().is_empty() {
        DEFAULT_PRESET
    } else {
        preset.trim()
    };
    let (argv, out_name) = plan(
        interval,
        unit,
        truthy(scene_cut),
        truthy(closed_gop),
        quality,
        preset,
        in_name,
    )
    .map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
