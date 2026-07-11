//! Browser-facing wasm-bindgen wrapper for /tools/mkv-to-mp4/ (ffmpeg page).
//! Builds the ffmpeg argv (pure, shared with the chat block via core); the JS
//! page driver runs it through the browser ffmpeg bridge.
//!
//! Page field order (meta.toml) MUST match this param order: `mode`, then
//! `quality`, then the file (`in_name`). `tool.js` calls
//! `build_argv(...fieldArgs, inName)`.

use wasm_bindgen::prelude::*;

use gizza_ai_block_utils::ArgvPlan;
use gizza_ai_mkv_to_mp4_core::{plan, DEFAULT_QUALITY};

/// `mode` is `copy|transcode`; `quality` is 1-100 (0/empty defaults to 75, and
/// only affects `transcode`). Returns `{ argv, out_name: "out.mp4" }` or throws a
/// JS error string.
#[wasm_bindgen]
pub fn build_argv(mode: &str, quality: f64, in_name: &str) -> Result<JsValue, JsValue> {
    let q = if quality > 0.0 {
        quality.round().clamp(1.0, 100.0) as u8
    } else {
        DEFAULT_QUALITY
    };
    let (argv, out_name) = plan(mode, q, in_name).map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
