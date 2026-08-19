//! Browser-facing wasm-bindgen wrapper for /tools/image-dither/ (ffmpeg page).
//! Builds the ffmpeg argv (pure, shared with the chat block via core); the JS
//! page driver runs it through the browser ffmpeg bridge.
//!
//! Page field order (meta.toml) MUST match this param order: `algorithm`,
//! `palette`, `colors`, `palette_colors`, `bayer_scale`, `pixel_scale`,
//! `contrast`, `format`, then the file (`in_name`). `tool.js` calls
//! `build_argv(...fieldArgs, inName)`.

use gizza_ai_block_utils::ArgvPlan;
use gizza_ai_image_dither_core::plan;
use wasm_bindgen::prelude::*;

/// Defaults used when a page field arrives empty (`tool.js` sends "" for text
/// fields and coerces empty numeric fields to 0.0).
const DEFAULT_ALGORITHM: &str = "floyd_steinberg";
const DEFAULT_PALETTE: &str = "auto";
const DEFAULT_FORMAT: &str = "png";
const DEFAULT_COLORS: u32 = 16;
const DEFAULT_PIXEL_SCALE: u32 = 1;
const DEFAULT_CONTRAST: f64 = 1.0;

fn or_default<'a>(v: &'a str, fallback: &'a str) -> &'a str {
    if v.trim().is_empty() {
        fallback
    } else {
        v
    }
}

/// `algorithm` is one of the nine `paletteuse` kernels; `palette` is
/// `auto|mono|gray4|gray16|green4|amber2|cga4|custom`; `colors` is 2-256 (only
/// used by `auto`); `palette_colors` is a comma-separated hex list (only used by
/// `custom`); `bayer_scale` is 0-5; `pixel_scale` is 1-16; `contrast` is
/// 0.5-3.0; `format` is `same|png|jpeg|webp|gif`. An empty numeric field arrives
/// as 0.0 and falls back to the documented default. Returns
/// `{ argv: string[], out_name }` or throws a JS error string.
#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn build_argv(
    algorithm: &str,
    palette: &str,
    colors: f64,
    palette_colors: &str,
    bayer_scale: f64,
    pixel_scale: f64,
    contrast: f64,
    format: &str,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    let algorithm = or_default(algorithm, DEFAULT_ALGORITHM);
    let palette = or_default(palette, DEFAULT_PALETTE);
    let format = or_default(format, DEFAULT_FORMAT);
    let colors = if colors > 0.0 {
        colors.round() as u32
    } else {
        DEFAULT_COLORS
    };
    // 0 is a legitimate bayer_scale (the finest matrix), so it is passed through
    // as-is; only negatives (which the page can't produce) are clamped away.
    let bayer_scale = bayer_scale.max(0.0).round() as u32;
    let pixel_scale = if pixel_scale > 0.0 {
        pixel_scale.round() as u32
    } else {
        DEFAULT_PIXEL_SCALE
    };
    let contrast = if contrast > 0.0 {
        contrast
    } else {
        DEFAULT_CONTRAST
    };

    let (argv, out_name) = plan(
        in_name,
        algorithm,
        palette,
        colors,
        palette_colors,
        bayer_scale,
        pixel_scale,
        contrast,
        format,
    )
    .map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
