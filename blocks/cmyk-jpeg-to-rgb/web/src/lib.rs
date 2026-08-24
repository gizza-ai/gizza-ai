//! Browser-facing wasm-bindgen wrapper for /tools/cmyk-jpeg-to-rgb/ (ffmpeg page).
//! Builds the ffmpeg argv (pure, shared with the chat block via core); returns
//! the shared block_utils::ArgvPlan so the page driver gets { argv, out_name }.
//!
//! Page field order (meta.toml) MUST match this param order: `format`,
//! `quality`, `chroma`, then the file (`in_name`). `tool.js` calls
//! `build_argv(...fieldArgs, inName)`.
use gizza_ai_block_utils::ArgvPlan;
use wasm_bindgen::prelude::*;

/// `format` is png|jpeg|webp (empty → png). `quality` is 1-100 for jpeg/webp (a
/// cleared field arrives as 0 → default 90). `chroma` is "4:2:0"|"4:4:4" (empty
/// → 4:2:0) and only affects jpeg. Returns `{ argv: string[], out_name }` or
/// throws a JS error string.
#[wasm_bindgen]
pub fn build_argv(
    format: &str,
    quality: f64,
    chroma: &str,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    let (argv, out_name) = gizza_ai_cmyk_jpeg_to_rgb_core::plan(in_name, format, quality, chroma)
        .map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
