//! Browser-facing wasm-bindgen wrapper for /tools/video-to-dnxhd/ (ffmpeg page).
//! Builds the ffmpeg argv (pure, shared with the chat block via core); returns
//! the shared block_utils::ArgvPlan so the page driver gets { argv, out_name }.
//!
//! Page field order (meta.toml) MUST match this param order: `profile`,
//! `container`, `resolution`, `pixel_format`, `audio`, then file (`in_name`).

use gizza_ai_block_utils::ArgvPlan;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn build_argv(
    profile: &str,
    container: &str,
    resolution: &str,
    pixel_format: &str,
    audio: &str,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    let p = if profile.is_empty() { "dnxhr_sq" } else { profile };
    let c = if container.is_empty() { "mov" } else { container };
    let r = if resolution.is_empty() { "source" } else { resolution };
    let pf = if pixel_format.is_empty() { "auto" } else { pixel_format };
    let a = if audio.is_empty() { "pcm16" } else { audio };
    let (argv, out_name) = gizza_ai_video_to_dnxhd_core::plan(p, c, r, pf, a, in_name)
        .map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
