//! Browser-facing wasm-bindgen wrapper for /tools/video-audio-sync-offset/ (ffmpeg page).
//! Builds the ffmpeg argv (pure, shared with the chat block via core); returns
//! the shared block_utils::ArgvPlan so the page driver gets { argv, out_name }.
use gizza_ai_block_utils::ArgvPlan;
use wasm_bindgen::prelude::*;

/// `offset` is a signed shift in `unit` (`ms`|`seconds`); positive delays the
/// audio, negative advances it. Field order matches `page/meta.toml`
/// (offset, unit, then in_name). Returns `{ argv, out_name }` or a JS error.
#[wasm_bindgen]
pub fn build_argv(offset: f64, unit: &str, in_name: &str) -> Result<JsValue, JsValue> {
    let unit = if unit.trim().is_empty() { "ms" } else { unit };
    let (argv, out_name) = gizza_ai_video_audio_sync_offset_core::plan(in_name, offset, unit)
        .map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
