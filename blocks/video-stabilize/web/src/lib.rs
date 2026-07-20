//! Browser-facing wasm-bindgen wrapper for /tools/video-stabilize/ (ffmpeg
//! page). Builds the ffmpeg argv (pure, shared with the chat block via core);
//! returns the shared block_utils::ArgvPlan so the page driver gets
//! { argv, out_name }.
//!
//! Page field order (meta.toml) MUST match this param order: `borders`, then
//! `strength`, then the file (`in_name`). `tool.js` calls
//! `build_argv(...fieldArgs, inName)`.
use gizza_ai_block_utils::ArgvPlan;
use gizza_ai_video_stabilize_core::plan;
use wasm_bindgen::prelude::*;

/// `borders` is `mirror|crop|blank|original` (empty → mirror); `strength` is
/// 1-100 (0/empty → core default 25, out-of-range is clamped). Returns
/// `{ argv, out_name }` or throws a JS error string. The page passes the two
/// fields then the uploaded file's `in_name` (field order = param order).
#[wasm_bindgen]
pub fn build_argv(borders: &str, strength: f64, in_name: &str) -> Result<JsValue, JsValue> {
    // Empty borders → the page/core default (mirror). A non-finite `strength`
    // (JS never sends one from a number input, but be defensive) collapses to
    // 0.0 so core resolves it to the default instead of erroring.
    let borders = if borders.trim().is_empty() {
        "mirror"
    } else {
        borders.trim()
    };
    let strength = if strength.is_finite() { strength } else { 0.0 };
    let (argv, out_name) = plan(strength, borders, in_name).map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
