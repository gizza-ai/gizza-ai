//! Browser-facing wasm-bindgen wrapper for the standalone /tools/video-frame-extract/
//! page. Builds the ffmpeg argv (pure, shared with the chat block via core); the
//! JS page driver runs it through the browser ffmpeg bridge.
//!
//! Input is a video file; output is always a PNG image (the page's `format` is
//! `image`, so the extracted frame renders as an `<img>`).

use wasm_bindgen::prelude::*;

use gizza_ai_block_utils::ArgvPlan;
use gizza_ai_video_frame_extract_core::plan_extract;

/// `timestamp` is in seconds (0 grabs the first frame). The page passes the
/// `timestamp` field then the uploaded file's `in_name` (field order =
/// `build_argv` param order). Returns `{ argv: string[], out_name }` (always
/// `out.png`) or throws a JS error string.
#[wasm_bindgen]
pub fn build_argv(timestamp: f64, in_name: &str) -> Result<JsValue, JsValue> {
    let (argv, out_name) =
        plan_extract(in_name, timestamp).map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
