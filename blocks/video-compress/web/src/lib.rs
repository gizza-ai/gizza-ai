//! Browser-facing wasm-bindgen wrapper for the standalone /tools/video-compress/
//! page. Builds the ffmpeg argv (pure, shared with the chat block via core); the
//! JS page driver runs it through the browser ffmpeg bridge.

use wasm_bindgen::prelude::*;

use gizza_ai_block_utils::ArgvPlan;

/// `crf` of 0 (or non-finite) means "unset" and falls back to the default; the
/// value is clamped to the accepted quality range by core. Returns
/// `{ argv: string[], out_name }`. The page passes the `crf` field then the
/// uploaded file's `in_name` (field order = `build_argv` param order).
#[wasm_bindgen]
pub fn build_argv(crf: f64, in_name: &str) -> Result<JsValue, JsValue> {
    let (argv, out_name) = gizza_ai_video_compress_core::build_argv(crf, in_name);
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
