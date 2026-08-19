//! Browser-facing wasm-bindgen wrapper for /tools/image-auto-orient/ (ffmpeg page).
//! Builds the ffmpeg argv (pure, shared with the chat block via core); the JS page
//! driver runs it through the browser ffmpeg bridge.
//!
//! Page field order (meta.toml) MUST match this param order: `orientation`, then
//! `format`, then `quality`, then the file (`in_name`). `tool.js` calls
//! `build_argv(...fieldArgs, inName)`.

use gizza_ai_block_utils::ArgvPlan;
use gizza_ai_image_auto_orient_core::plan;
use wasm_bindgen::prelude::*;

/// Defaults used when a page field arrives empty (`tool.js` sends "" for text
/// fields and coerces empty numeric fields to 0.0).
const DEFAULT_ORIENTATION: &str = "auto";
const DEFAULT_FORMAT: &str = "same";
const DEFAULT_QUALITY: u8 = 90;

/// `orientation` is `auto` or `1`-`8`; `format` is `same|jpeg|png|webp`;
/// `quality` is 1-100 (0/empty defaults to 90, ignored for png). Returns
/// `{ argv: string[], out_name }` or throws a JS error string.
#[wasm_bindgen]
pub fn build_argv(
    orientation: &str,
    format: &str,
    quality: f64,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    let orientation = if orientation.trim().is_empty() {
        DEFAULT_ORIENTATION
    } else {
        orientation
    };
    let format = if format.trim().is_empty() {
        DEFAULT_FORMAT
    } else {
        format
    };
    let quality = if quality > 0.0 {
        quality.round().clamp(1.0, 100.0) as u8
    } else {
        DEFAULT_QUALITY
    };
    let (argv, out_name) =
        plan(orientation, format, quality, in_name).map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
