//! Browser-facing wasm-bindgen wrapper for /tools/video-dedup-frames/ (ffmpeg
//! page). Builds the ffmpeg argv (pure, shared with the chat block via core);
//! returns the shared block_utils::ArgvPlan so the page driver gets
//! { argv, out_name }.
//!
//! Page field order (meta.toml) MUST match this param order: `sensitivity`,
//! `timing`, `max_fps`, `format`, `frac`, then the file (`in_name`). `tool.js`
//! calls `build_argv(...fieldArgs, inName)`.
use gizza_ai_block_utils::ArgvPlan;
use gizza_ai_video_dedup_frames_core::plan;
use wasm_bindgen::prelude::*;

/// `sensitivity` is 1–100 (0/empty → core default 50), `timing` is
/// `keep|constant|compact` (empty → keep), `max_fps` is 1–240 (0/empty → keep
/// the source rate), `format` is `auto|mp4|webm` (empty → auto) and `frac` is
/// 0.01–1 (0/empty → core default 0.33). Returns `{ argv, out_name }` or throws
/// a JS error string.
#[wasm_bindgen]
pub fn build_argv(
    sensitivity: f64,
    timing: &str,
    max_fps: f64,
    format: &str,
    frac: f64,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    // Empty selects fall back to the page/core defaults. A non-finite number
    // (an empty number field can reach us as NaN) collapses to the 0 "unset"
    // sentinel so core resolves it to the default instead of erroring.
    let timing = if timing.trim().is_empty() {
        "keep"
    } else {
        timing.trim()
    };
    let format = if format.trim().is_empty() {
        "auto"
    } else {
        format.trim()
    };
    let num = |v: f64| if v.is_finite() { v } else { 0.0 };
    let (argv, out_name) = plan(
        num(sensitivity),
        timing,
        num(max_fps),
        format,
        num(frac),
        in_name,
    )
    .map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
