//! Browser-facing wasm-bindgen wrapper for /tools/image-convert/ (ffmpeg page).
//! Builds the ffmpeg argv (pure, shared with the chat block via core).
//!
//! Page field order (meta.toml) MUST match this param order: `format`, then
//! `quality`, then the file (`in_name`). `tool.js` calls
//! `build_argv(...fieldArgs, inName)`.

use wasm_bindgen::prelude::*;

use gizza_ai_block_utils::ArgvPlan;
use gizza_ai_image_convert_core::plan_convert;

/// Default quality used when the page's quality field is left blank (which
/// arrives here as 0.0 — see `tool.js` numeric coercion of empty fields).
const DEFAULT_QUALITY: u8 = 85;

/// `format` is one of `jpeg|png|webp`; `quality` is 1-100 (0/empty defaults to
/// 85, ignored for png). Returns `{ argv: string[], out_name }` or throws a JS
/// error string.
#[wasm_bindgen]
pub fn build_argv(format: &str, quality: f64, in_name: &str) -> Result<JsValue, JsValue> {
    let q = if quality > 0.0 {
        quality.round().clamp(1.0, 100.0) as u8
    } else {
        DEFAULT_QUALITY
    };
    let (argv, out_name) = plan_convert(format, q, in_name).map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
