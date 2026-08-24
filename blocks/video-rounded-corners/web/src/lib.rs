//! Browser-facing wasm-bindgen wrapper for /tools/video-rounded-corners/ (ffmpeg
//! page). Builds the ffmpeg argv (pure, shared with the chat block via core);
//! returns the shared block_utils::ArgvPlan so the page driver gets
//! { argv, out_name }.
//!
//! Page field order (meta.toml) MUST match this param order: `radius`,
//! `radius_unit`, `corners`, `background`, `format`, `quality`, `keep_audio`,
//! then the file (`in_name`). `tool.js` calls `build_argv(...fieldArgs, inName)`.
use gizza_ai_block_utils::ArgvPlan;
use gizza_ai_video_rounded_corners_core::{plan, DEFAULT_QUALITY, DEFAULT_RADIUS};
use wasm_bindgen::prelude::*;

/// `radius` is the corner radius (0/empty → the core default 24); `radius_unit`
/// is `px|percent` (empty → px); `corners` is `all|top|bottom|left|right`
/// (empty → all); `background` is `transparent` or a CSS color/hex (empty →
/// transparent); `format` is `webm|mp4|mov` (empty → webm); `quality` is 1-100
/// (0/empty → 75); `keep_audio` is the checkbox state (`"true"`/`"false"`).
/// Returns `{ argv, out_name }` or throws a JS error string.
#[wasm_bindgen]
pub fn build_argv(
    radius: f64,
    radius_unit: &str,
    corners: &str,
    background: &str,
    format: &str,
    quality: f64,
    keep_audio: &str,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    // An empty numeric field arrives as 0 — fall back to the descriptor default
    // rather than building a no-op (radius 0) or an invalid (quality 0) plan.
    let radius = if radius.is_finite() && radius > 0.0 {
        radius
    } else {
        DEFAULT_RADIUS
    };
    let quality = if quality.is_finite() && quality > 0.0 {
        quality.round() as u32
    } else {
        DEFAULT_QUALITY
    };
    // Checkboxes arrive as "true"/"false"; anything positive-truthy counts.
    let keep_audio = matches!(
        keep_audio.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    );

    let (argv, out_name) = plan(
        in_name,
        radius,
        radius_unit,
        corners,
        background,
        format,
        quality,
        keep_audio,
    )
    .map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
