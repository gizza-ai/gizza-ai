//! Browser-facing wasm-bindgen wrapper for /tools/video-denoise/ (ffmpeg page).
//! Builds the ffmpeg argv (pure, shared with the chat block via core); returns
//! the shared block_utils::ArgvPlan so the page driver gets { argv, out_name }.
//!
//! Page field order (meta.toml) MUST match this param order: `method`, then
//! `strength`, then the file (`in_name`). `tool.js` calls
//! `build_argv(...fieldArgs, inName)`.
use gizza_ai_block_utils::ArgvPlan;
use gizza_ai_video_denoise_core::plan;
use wasm_bindgen::prelude::*;

/// `method` is `hqdn3d|nlmeans` (empty → hqdn3d); `strength` is 1-100
/// (0/empty → core default 30, out-of-range is clamped). Returns
/// `{ argv, out_name }` or throws a JS error string. The page passes the two
/// fields then the uploaded file's `in_name` (field order = param order).
#[wasm_bindgen]
pub fn build_argv(method: &str, strength: f64, in_name: &str) -> Result<JsValue, JsValue> {
    // Empty method → the page/core default (hqdn3d). A non-finite `strength`
    // (JS never sends one from a number input, but be defensive) collapses to
    // 0.0 so core resolves it to the default instead of erroring.
    let method = if method.trim().is_empty() {
        "hqdn3d"
    } else {
        method.trim()
    };
    let strength = if strength.is_finite() { strength } else { 0.0 };
    let (argv, out_name) = plan(strength, method, in_name).map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
