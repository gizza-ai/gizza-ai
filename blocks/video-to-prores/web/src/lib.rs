//! Browser-facing wasm-bindgen wrapper for /tools/video-to-prores/ (ffmpeg page).
//! Builds the ffmpeg argv (pure, shared with the chat block via core); returns
//! the shared block_utils::ArgvPlan so the page driver gets { argv, out_name }.
//!
//! Page field order (meta.toml) MUST match this param order: `profile`, then
//! `resolution`, then `audio`, then the file (`in_name`). `tool.js` calls
//! `build_argv(...fieldArgs, inName)`.

use gizza_ai_block_utils::ArgvPlan;
use wasm_bindgen::prelude::*;

/// Build ProRes 422 MOV ffmpeg argv for the page runtime.
#[wasm_bindgen]
pub fn build_argv(
    profile: &str,
    resolution: &str,
    audio: &str,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    let p = if profile.is_empty() { "standard" } else { profile };
    let r = if resolution.is_empty() { "source" } else { resolution };
    let a = if audio.is_empty() { "pcm16" } else { audio };
    let (argv, out_name) =
        gizza_ai_video_to_prores_core::plan(p, r, a, in_name).map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
