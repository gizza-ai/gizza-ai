//! Browser-facing wasm-bindgen wrapper for /tools/seamless-loop-video/ (ffmpeg
//! page). Builds the ffmpeg argv (pure, shared with the chat block via core);
//! the JS page driver runs it through the browser ffmpeg bridge.
//!
//! Page field order (meta.toml) MUST match this param order: `crossfade`, then
//! `quality`, then the file (`in_name`). `tool.js` calls
//! `build_argv(...fieldArgs, inName)`.

use wasm_bindgen::prelude::*;

use gizza_ai_block_utils::ArgvPlan;
use gizza_ai_seamless_loop_video_core::{plan, DEFAULT_CROSSFADE, DEFAULT_QUALITY};

/// `crossfade` is the overlap length in seconds (0/empty defaults to 0.5);
/// `quality` is 1-100 (0/empty defaults to 75). Always re-encodes to a silent
/// H.264/MP4. Returns `{ argv, out_name: "out.mp4" }` or throws a JS error
/// string (e.g. crossfade out of range).
#[wasm_bindgen]
pub fn build_argv(crossfade: f64, quality: f64, in_name: &str) -> Result<JsValue, JsValue> {
    let x = if crossfade > 0.0 {
        crossfade
    } else {
        DEFAULT_CROSSFADE
    };
    let q = if quality > 0.0 {
        quality.round().clamp(1.0, 100.0) as u8
    } else {
        DEFAULT_QUALITY
    };
    let (argv, out_name) = plan(x, q, in_name).map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
