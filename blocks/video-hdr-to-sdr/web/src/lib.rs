//! Browser-facing wasm-bindgen wrapper for /tools/video-hdr-to-sdr/ (ffmpeg page).
//! Builds the ffmpeg argv (pure, shared with the chat block via core); returns
//! the shared block_utils::ArgvPlan so the page driver gets { argv, out_name }.
//!
//! Page field order (meta.toml) MUST match this param order: `tonemap`, `peak`,
//! `desat`, `format`, `quality`, then the file (`in_name`). `tool.js` calls
//! `build_argv(...fieldArgs, inName)`.

use gizza_ai_block_utils::ArgvPlan;
use gizza_ai_video_hdr_to_sdr_core::{
    plan_hdr_to_sdr, DEFAULT_ALGORITHM, DEFAULT_DESAT, DEFAULT_FORMAT, DEFAULT_PEAK,
    DEFAULT_QUALITY,
};
use wasm_bindgen::prelude::*;

/// `tonemap` is one of `hable|mobius|reinhard|linear|clip`; `peak` is the target
/// nominal peak luminance in nits (100-10000, 0/empty → 100); `desat` is highlight
/// desaturation 0-4 (empty → 0); `format` is `mp4|webm`; `quality` is 1-100
/// (0/empty → 75). Returns `{ argv, out_name }` or throws a JS error string.
#[wasm_bindgen]
pub fn build_argv(
    tonemap: &str,
    peak: f64,
    desat: f64,
    format: &str,
    quality: f64,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    let tonemap = if tonemap.trim().is_empty() {
        DEFAULT_ALGORITHM.as_arg()
    } else {
        tonemap.trim()
    };
    let format = if format.trim().is_empty() {
        DEFAULT_FORMAT.ext()
    } else {
        format.trim()
    };
    let peak = if peak > 0.0 {
        peak.round().clamp(100.0, 10000.0) as u32
    } else {
        DEFAULT_PEAK
    };
    let desat = if desat > 0.0 {
        desat.clamp(0.0, 4.0)
    } else {
        DEFAULT_DESAT
    };
    let quality = if quality > 0.0 {
        quality.round().clamp(1.0, 100.0) as u8
    } else {
        DEFAULT_QUALITY
    };
    let (argv, out_name) = plan_hdr_to_sdr(tonemap, peak, desat, format, quality, in_name)
        .map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
