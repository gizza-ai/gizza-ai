//! Browser-facing wasm-bindgen wrapper for /tools/bmp-converter/ (ffmpeg page).
//! Builds the ffmpeg argv (pure, shared with the chat block via core); returns
//! the shared block_utils::ArgvPlan so the page driver gets { argv, out_name }.
//!
//! Page field order (meta.toml) MUST match this param order: `format`,
//! `bit_depth`, `colors`, `dither`, `grayscale`, `background`, then the file
//! (`in_name`). `tool.js` calls `build_argv(...fieldArgs, inName)`.
use gizza_ai_block_utils::ArgvPlan;
use gizza_ai_bmp_converter_core::plan_named;
use wasm_bindgen::prelude::*;

/// `format` is `bmp|png|jpeg` (empty → bmp). `bit_depth` is
/// `1|8|16-555|16-565|24|32` (empty → 24). `colors` is the 8-bit palette size
/// 2–256 (a CLEARED page field arrives as 0 → the default 256). `dither` is
/// `none|bayer|floyd_steinberg|sierra2_4a` (empty → floyd_steinberg).
/// `grayscale` is a checkbox string ("true"/"false"; empty → false).
/// `background` is a hex colour or ffmpeg colour name (empty → #ffffff).
/// Returns `{ argv: string[], out_name }` or throws a JS error string.
#[wasm_bindgen]
pub fn build_argv(
    format: &str,
    bit_depth: &str,
    colors: f64,
    dither: &str,
    grayscale: &str,
    background: &str,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    let (argv, out_name) = plan_named(
        in_name,
        Some(format),
        Some(bit_depth),
        colors,
        Some(dither),
        Some(grayscale),
        Some(background),
    )
    .map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
