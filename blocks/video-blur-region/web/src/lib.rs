//! Browser-facing wasm-bindgen wrapper for /tools/video-blur-region/ (ffmpeg page).
//! Builds the ffmpeg argv (pure, shared with the chat block via core); returns
//! the shared block_utils::ArgvPlan so the page driver gets { argv, out_name }.
//!
//! Page field order (meta.toml) MUST match this param order: `x`, `y`, `width`,
//! `height`, `mode`, `strength`, then the file (`in_name`). `tool.js` calls
//! `build_argv(...fieldArgs, inName)`.
use gizza_ai_block_utils::ArgvPlan;
use gizza_ai_video_blur_region_core::{plan, Mode};
use wasm_bindgen::prelude::*;

/// `x`/`y` are the region's top-left offset in pixels (empty → 0);
/// `width`/`height` are the region size in pixels (must be > 0); `mode` is
/// `blur|pixelate` (empty → blur); `strength` is 1-100 (0/empty → the core
/// default 20). Returns `{ argv, out_name }` or throws a JS error string. The
/// page passes the six fields then the uploaded file's `in_name` (field order =
/// param order).
#[wasm_bindgen]
pub fn build_argv(
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    mode: &str,
    strength: f64,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    // Empty mode → the page/core default (blur). A non-finite or non-positive
    // strength (JS never sends one from the slider, but be defensive) collapses
    // to the descriptor default 20.
    let mode = if mode.trim().is_empty() { "blur" } else { mode.trim() };
    let mode = Mode::parse(mode).map_err(|e| JsValue::from_str(&e))?;
    let non_neg = |v: f64| if v.is_finite() && v > 0.0 { v as u32 } else { 0 };
    let strength = if strength.is_finite() && strength > 0.0 { strength as u32 } else { 20 };

    let (argv, out_name) = plan(
        in_name,
        non_neg(x),
        non_neg(y),
        non_neg(width),
        non_neg(height),
        mode,
        strength,
    )
    .map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
