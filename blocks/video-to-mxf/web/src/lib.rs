//! Browser-facing wasm-bindgen wrapper for /tools/video-to-mxf/ (ffmpeg page).
//! Builds the ffmpeg argv (pure, shared with the chat block via core); returns
//! the shared block_utils::ArgvPlan so the page driver gets { argv, out_name }.
//!
//! Page field order (meta.toml) MUST match this param order: `profile`,
//! `resolution`, `frame_rate`, `audio`, then file (`in_name`).

use gizza_ai_block_utils::ArgvPlan;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn build_argv(
    profile: &str,
    resolution: &str,
    frame_rate: &str,
    audio: &str,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    let p = if profile.is_empty() { "xdcam_hd422" } else { profile };
    let r = if resolution.is_empty() { "auto" } else { resolution };
    let f = if frame_rate.is_empty() { "source" } else { frame_rate };
    let a = if audio.is_empty() { "pcm16" } else { audio };
    let (argv, out_name) = gizza_ai_video_to_mxf_core::plan(p, r, f, a, in_name)
        .map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
