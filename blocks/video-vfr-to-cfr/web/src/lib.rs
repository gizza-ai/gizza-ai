//! Browser-facing wasm-bindgen wrapper for the standalone /tools/video-vfr-to-cfr/
//! page. Builds the ffmpeg argv (pure, shared with the chat block via core);
//! returns the shared `block_utils::ArgvPlan` so the page driver gets
//! `{ argv, out_name }`.
//!
//! Page field order (meta.toml) MUST match this param order: `fps`, then
//! `quality`, then `resync_audio`, then the file (`in_name`). `tool.js` calls
//! `build_argv(...fieldArgs, inName)`.

use gizza_ai_block_utils::ArgvPlan;
use wasm_bindgen::prelude::*;

/// `fps` is `auto|23.976|24|25|29.97|30|50|59.94|60` (empty → auto), `quality`
/// is `high|balanced|small` (empty → balanced), and `resync_audio` is the
/// checkbox value string — positive-truthy, and an EMPTY string means "field
/// absent", which keeps the advertised default (on). Returns
/// `{ argv, out_name }` or throws a JS error string.
#[wasm_bindgen]
pub fn build_argv(
    fps: &str,
    quality: &str,
    resync_audio: &str,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    let resync = match resync_audio.trim() {
        "" => gizza_ai_video_vfr_to_cfr_core::DEFAULT_RESYNC_AUDIO,
        v => matches!(v.to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes"),
    };
    let (argv, out_name) = gizza_ai_video_vfr_to_cfr_core::plan(fps, quality, resync, in_name)
        .map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
