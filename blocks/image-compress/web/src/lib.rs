//! Browser-facing wasm-bindgen wrapper for /tools/image-compress/ (ffmpeg page).
//! Builds the ffmpeg argv (pure, shared with the chat block via core).
//!
//! Page field order (meta.toml) MUST match this param order: `quality`, then the
//! file (`in_name`). `tool.js` calls `build_argv(...fieldArgs, inName)`.
use serde::Serialize;
use wasm_bindgen::prelude::*;

use gizza_ai_image_compress_core::plan_compress;

/// Default quality used when the page's quality field is left blank (which
/// arrives here as 0.0 — see `tool.js` numeric coercion of empty fields).
const DEFAULT_QUALITY: u8 = 80;

#[derive(Serialize)]
struct Plan {
    argv: Vec<String>,
    out_name: String,
}

/// `quality` is 1-100; 0 (an empty field) defaults to 80. Returns
/// `{ argv: string[], out_name }` or throws a JS error string.
#[wasm_bindgen]
pub fn build_argv(quality: f64, in_name: &str) -> Result<JsValue, JsValue> {
    let q = if quality > 0.0 {
        // Clamp into u8 range before handing to the validated core.
        quality.round().clamp(1.0, 100.0) as u8
    } else {
        DEFAULT_QUALITY
    };
    let (argv, out_name) = plan_compress(q, in_name).map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&Plan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
