//! Browser-facing wasm-bindgen wrapper for /tools/video-to-gif/ (ffmpeg page).
//! Builds the ffmpeg argv (pure, shared with the chat block via core); the JS
//! page driver runs it through the browser ffmpeg bridge.
//!
//! Page field order (meta.toml) MUST match this param order: `start`,
//! `duration`, `fps`, `width`, then the file (`in_name`). `tool.js` calls
//! `build_argv(...fieldArgs, inName)`.

use wasm_bindgen::prelude::*;

use gizza_ai_block_utils::ArgvPlan;
use gizza_ai_video_to_gif_core::plan;

/// `start`/`duration` window the clip (seconds; 0 = beginning / to the end);
/// `fps` thins frames (0 = default 12); `width` resizes (0 = keep source size).
/// Empty page fields arrive here as `0.0`. Returns `{ argv, out_name }` (always
/// `out.gif`) or throws a JS error string.
#[wasm_bindgen]
pub fn build_argv(
    start: f64,
    duration: f64,
    fps: f64,
    width: f64,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    let (argv, out_name) =
        plan(in_name, start, duration, fps, width).map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
