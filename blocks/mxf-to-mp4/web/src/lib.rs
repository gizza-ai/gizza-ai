//! Browser-facing wasm-bindgen wrapper for /tools/mxf-to-mp4/ (ffmpeg page).
//! Builds the ffmpeg argv (pure, shared with the chat block via core); the JS
//! page driver runs it through the browser ffmpeg bridge.
//!
//! Page field order (meta.toml) MUST match this param order: `video`, `quality`,
//! `audio`, `merge_tracks`, `audio_bitrate`, then the file (`in_name`).
//! `tool.js` calls `build_argv(...fieldArgs, inName)`.

use wasm_bindgen::prelude::*;

use gizza_ai_block_utils::ArgvPlan;
use gizza_ai_mxf_to_mp4_core::{
    plan, DEFAULT_AUDIO_BITRATE, DEFAULT_MERGE_TRACKS, DEFAULT_QUALITY,
};

/// `video` is `h264|rewrap`; `quality` is 1-100 (0/empty defaults to 75, and only
/// affects `h264`); `audio` is `stereo|merge|all|none`; `merge_tracks` is 2-16
/// (0/empty defaults to 2, only used when `audio = "merge"`); `audio_bitrate` is
/// 32-320 kbps (0/empty defaults to 192). Numeric fields arrive as `f64` — an
/// empty page field is 0, which means "use the default" rather than an error.
/// Returns `{ argv, out_name: "out.mp4" }` or throws a JS error string.
#[wasm_bindgen]
pub fn build_argv(
    video: &str,
    quality: f64,
    audio: &str,
    merge_tracks: f64,
    audio_bitrate: f64,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    let q = if quality > 0.0 {
        quality.round().clamp(1.0, 100.0) as u8
    } else {
        DEFAULT_QUALITY
    };
    // Out-of-range values are passed through to `plan`, which rejects them with a
    // message naming the accepted range — only "empty" is defaulted here.
    let tracks = if merge_tracks > 0.0 {
        merge_tracks.round().clamp(0.0, 255.0) as u8
    } else {
        DEFAULT_MERGE_TRACKS
    };
    let bitrate = if audio_bitrate > 0.0 {
        audio_bitrate.round().clamp(0.0, 65535.0) as u16
    } else {
        DEFAULT_AUDIO_BITRATE
    };
    let (argv, out_name) =
        plan(video, q, audio, tracks, bitrate, in_name).map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
