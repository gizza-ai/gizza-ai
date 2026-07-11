//! Browser-facing wasm-bindgen wrapper for /tools/video-cut-segments/ (ffmpeg page).
//! Builds the ffmpeg argv (pure, shared with the chat block via core); the JS
//! page driver runs it through the browser ffmpeg bridge.
//!
//! Page field order in meta.toml MUST equal this param order: `segments`, then
//! `mode`, then the uploaded file's `in_name`. `tool.js` calls
//! `build_argv(...fieldArgs, inName)`.
use gizza_ai_block_utils::ArgvPlan;
use wasm_bindgen::prelude::*;

/// `segments` is the comma/newline list of `start-end` windows; `mode` is
/// `keep` (join only those windows, default) or `remove` (cut them out, keep the
/// rest). Returns `{ argv, out_name }` (always `out.mp4` — the join re-encodes)
/// or a JS error string on a bad segment list / mode.
#[wasm_bindgen]
pub fn build_argv(segments: &str, mode: &str, in_name: &str) -> Result<JsValue, JsValue> {
    let mode = if mode.trim().is_empty() { "keep" } else { mode };
    let (argv, out_name) = gizza_ai_video_cut_segments_core::plan(in_name, segments, mode)
        .map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
