//! Browser-facing wasm-bindgen wrapper for /tools/video-fade/ (ffmpeg page).
//! Builds the ffmpeg argv (pure, shared with the chat block via core); returns
//! the shared block_utils::ArgvPlan so the page driver gets { argv, out_name }.
use gizza_ai_block_utils::ArgvPlan;
use wasm_bindgen::prelude::*;

/// Field order matches `page/meta.toml` (fade_in, fade_out, duration, streams,
/// color, quality, then in_name). `duration` is the clip's exact length in
/// seconds and is only required when `fade_out` > 0. Returns `{ argv, out_name }`
/// or a JS error carrying the validation message shown to the user.
#[wasm_bindgen]
pub fn build_argv(
    fade_in: f64,
    fade_out: f64,
    duration: f64,
    streams: &str,
    color: &str,
    quality: &str,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    let (argv, out_name) = gizza_ai_video_fade_core::plan(
        in_name, fade_in, fade_out, duration, streams, color, quality,
    )
    .map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
