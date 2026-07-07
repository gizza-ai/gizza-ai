//! Browser-facing wasm-bindgen wrapper for the standalone
//! /tools/video-target-filesize-encoder/ page. Builds the ffmpeg argv (pure,
//! shared with the chat block via core); the page's `custom.js` reads the
//! `<video>.duration`, calls this, and runs the plan through the browser ffmpeg
//! bridge.

use gizza_ai_block_utils::ArgvPlan;
use wasm_bindgen::prelude::*;

/// `target_mb` is the file-size budget; `duration_s` is the clip length the page
/// read from the uploaded `<video>` element; `audio` / `scale` are the
/// `audio_kbps` / `scale` choices (none/64/96/128/192/320 and keep/1080/720/480/360).
/// Returns `{ argv, out_name }` or an error string the page shows verbatim.
#[wasm_bindgen]
pub fn build_argv(
    target_mb: f64,
    duration_s: f64,
    audio: &str,
    scale: &str,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    let (argv, out_name) = gizza_ai_video_target_filesize_encoder_core::build_argv(
        target_mb, duration_s, audio, scale, in_name,
    )
    .map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
