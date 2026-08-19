//! Browser-facing wasm-bindgen wrapper for /tools/video-fragmented-mp4/.
//! Builds the ffmpeg argv plan consumed by the shared ffmpeg page driver.
use gizza_ai_block_utils::ArgvPlan;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn build_argv(
    mode: &str,
    profile: &str,
    fragment_duration: f64,
    keyframe_interval: f64,
    segment_index: &str,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    let mode = if mode.trim().is_empty() { "copy" } else { mode };
    let profile = if profile.trim().is_empty() {
        "mse"
    } else {
        profile
    };
    let keyframe_interval = if keyframe_interval.is_finite() {
        keyframe_interval
    } else {
        gizza_ai_video_fragmented_mp4_core::DEFAULT_KEYFRAME_INTERVAL
    };
    let segment_index = matches!(
        segment_index.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    );
    let (argv, out_name) = gizza_ai_video_fragmented_mp4_core::plan(
        mode,
        profile,
        fragment_duration,
        keyframe_interval,
        segment_index,
        in_name,
    )
    .map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
