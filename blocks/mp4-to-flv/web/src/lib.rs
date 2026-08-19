//! Browser-facing wasm-bindgen wrapper for /tools/mp4-to-flv/ (ffmpeg page).
//! Builds the ffmpeg argv (pure, shared with the chat block via core); returns
//! the shared block_utils::ArgvPlan so the page driver gets { argv, out_name }.
//!
//! Page field order (`page/meta.toml` `[[input]] source = "field"`) MUST match
//! this param order — `tool.js` calls `build_argv(...fieldArgs, inName)`:
//! `video_codec`, `resolution`, `fps`, `video_bitrate`, `keyframe_seconds`,
//! `audio_codec`, `audio_bitrate`, then the uploaded file name.
//!
//! The three numeric fields arrive as JS numbers: the ffmpeg page driver
//! numeric-coerces numeric-LOOKING field strings before calling the export.
//! The enum values carry `p`/`fps` suffixes precisely so THEY never get coerced.

use gizza_ai_block_utils::ArgvPlan;
use wasm_bindgen::prelude::*;

/// `video_codec` is `h264|flv1`; `resolution` is `source|1080p|720p|576p|480p|360p|240p`;
/// `fps` is `source|60fps|30fps|25fps|24fps|15fps`; `video_bitrate` (100-20000) and
/// `audio_bitrate` (32-320) are kbps; `keyframe_seconds` is 1-10; `audio_codec` is
/// `aac|mp3|none`. Returns `{ argv, out_name }` (always `out.flv`) or throws the
/// core's error string, which the page shows verbatim.
#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn build_argv(
    video_codec: &str,
    resolution: &str,
    fps: &str,
    video_bitrate: f64,
    keyframe_seconds: f64,
    audio_codec: &str,
    audio_bitrate: f64,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    let (argv, out_name) = gizza_ai_mp4_to_flv_core::plan(
        video_codec,
        resolution,
        fps,
        video_bitrate,
        keyframe_seconds,
        audio_codec,
        audio_bitrate,
        in_name,
    )
    .map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
