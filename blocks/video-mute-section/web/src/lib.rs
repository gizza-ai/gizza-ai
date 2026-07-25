//! Browser-facing wasm-bindgen wrapper for /tools/video-mute-section/ (ffmpeg page).
//! Builds the ffmpeg argv (pure, shared with the chat block via core); returns
//! the shared block_utils::ArgvPlan so the page driver gets { argv, out_name }.
//!
//! Field order (meta.toml) MUST match this param order: `start`, `end`, then the
//! file (`in_name`). `tool.js` calls `build_argv(start, end, in_name)`.
use gizza_ai_block_utils::ArgvPlan;
use wasm_bindgen::prelude::*;

/// `start`/`end` are the silenced range in seconds (end must be > start).
/// Returns `{ argv, out_name }` or a JS error string.
#[wasm_bindgen]
pub fn build_argv(start: f64, end: f64, in_name: &str) -> Result<JsValue, JsValue> {
    let (argv, out_name) = gizza_ai_video_mute_section_core::plan(in_name, start, end)
        .map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
