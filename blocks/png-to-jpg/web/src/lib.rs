//! Browser-facing wasm-bindgen wrapper for /tools/png-to-jpg/ (ffmpeg page).
//! Builds the ffmpeg argv (pure, shared with the chat block via core); returns
//! the shared block_utils::ArgvPlan so the page driver gets { argv, out_name }.
//!
//! Page field order (meta.toml) MUST match this param order: `background`,
//! `quality`, then the file (`in_name`). `tool.js` calls
//! `build_argv(...fieldArgs, inName)`.
use gizza_ai_block_utils::ArgvPlan;
use wasm_bindgen::prelude::*;

/// `background` is the fill color for transparent areas (name or hex; empty →
/// #ffffff white). `quality` is 1-100 (a cleared field arrives as 0 → default
/// 85). Returns `{ argv: string[], out_name: "out.jpg" }` or throws a JS error
/// string.
#[wasm_bindgen]
pub fn build_argv(background: &str, quality: f64, in_name: &str) -> Result<JsValue, JsValue> {
    let (argv, out_name) = gizza_ai_png_to_jpg_core::plan(in_name, Some(background), quality)
        .map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
