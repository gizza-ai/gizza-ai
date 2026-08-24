//! Browser-facing wasm-bindgen wrapper for /tools/still-to-clip/ (ffmpeg page).
//! Builds the ffmpeg argv (pure, shared with the chat block via core); returns
//! the shared block_utils::ArgvPlan so the page driver gets { argv, out_name }.
//!
//! The page driver calls `build_argv(...fieldArgs, in_name)`, so the field order
//! here MUST equal the `[[input]]` field order in `page/meta.toml`
//! (duration, width, height, fit, background, fps, format, quality). Empty
//! numeric fields arrive as `0` (JS `Number("")` → 0), so a `0`/blank value
//! means "use the default" — the same contract the chat block applies in
//! `resolved()`. `background` is a `kind = "color"` field, which the ffmpeg page
//! runtime exempts from numeric coercion, so a hex value reaches us as a string.
use gizza_ai_block_utils::ArgvPlan;
use gizza_ai_still_to_clip_core::{
    normalize_color, plan, DEFAULT_DURATION, DEFAULT_FIT, DEFAULT_FORMAT, DEFAULT_FPS,
    DEFAULT_HEIGHT, DEFAULT_QUALITY, DEFAULT_WIDTH,
};
use wasm_bindgen::prelude::*;

/// Blank select/text fields mean "use the default" — same contract as chat.
fn or_default<'a>(v: &'a str, default: &'a str) -> &'a str {
    if v.trim().is_empty() {
        default
    } else {
        v
    }
}

#[wasm_bindgen]
pub fn build_argv(
    duration: f64,
    width: f64,
    height: f64,
    fit: &str,
    background: &str,
    fps: f64,
    format: &str,
    quality: f64,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    let duration = if duration > 0.0 { duration } else { DEFAULT_DURATION };
    let width = if width > 0.0 { width.round() as u32 } else { DEFAULT_WIDTH };
    let height = if height > 0.0 { height.round() as u32 } else { DEFAULT_HEIGHT };
    let fps = if fps > 0.0 { fps } else { DEFAULT_FPS };
    let quality = if quality > 0.0 {
        quality.round().clamp(0.0, 255.0) as u8
    } else {
        DEFAULT_QUALITY
    };
    let fit = or_default(fit, DEFAULT_FIT);
    let format = or_default(format, DEFAULT_FORMAT);
    let background = normalize_color(background).map_err(|e| JsValue::from_str(&e))?;

    let (argv, out_name) = plan(
        duration,
        width,
        height,
        fit,
        &background,
        fps,
        format,
        quality,
        in_name,
    )
    .map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
