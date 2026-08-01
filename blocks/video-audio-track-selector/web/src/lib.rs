//! Browser-facing wasm-bindgen wrapper for /tools/video-audio-track-selector/
//! (ffmpeg page). Builds the ffmpeg argv (pure, shared with the chat block via
//! core); the JS page driver runs it through the browser ffmpeg bridge.
//!
//! Page field order (meta.toml) MUST match this param order: `track`, then
//! `keep_subtitles`, then the file (`in_name`). `tool.js` calls
//! `build_argv(...fieldArgs, inName)`. The kept track is always flagged as the
//! default audio disposition on the page (`set_default` is on) — the rarely-toggled
//! disposition switch is exposed only on the CLI/chat descriptor.
use gizza_ai_block_utils::ArgvPlan;
use gizza_ai_video_audio_track_selector_core::plan;
use wasm_bindgen::prelude::*;

/// `track` arrives as a number (an empty field → `0.0`, the first-track default);
/// `keep_subtitles` is a checkbox value string (positive truthy). The kept audio
/// track is always flagged as the default disposition. Returns `{ argv, out_name }`
/// or throws a JS error string.
#[wasm_bindgen]
pub fn build_argv(
    track: f64,
    keep_subtitles: &str,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    if track < 0.0 || track.fract() != 0.0 {
        return Err(JsValue::from_str("track must be a whole number ≥ 0 (0 = first audio track)"));
    }
    let keep_subs = matches!(keep_subtitles, "true" | "1" | "on" | "yes");
    let (argv, out_name) =
        plan(in_name, track as u32, keep_subs, true).map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
