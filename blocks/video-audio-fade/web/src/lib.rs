//! Browser-facing wasm-bindgen wrapper for /tools/video-audio-fade/ (ffmpeg page).
//! Builds the ffmpeg argv (pure, shared with the chat block via core); returns
//! the shared block_utils::ArgvPlan so the page driver gets { argv, out_name }.
use gizza_ai_block_utils::ArgvPlan;
use wasm_bindgen::prelude::*;

/// `fade_in`/`fade_out` are per-side fade lengths in seconds (0 skips a side;
/// both 0 is rejected). Field order matches `page/meta.toml` (fade_in, fade_out,
/// then in_name). Returns `{ argv, out_name }` or a JS error.
#[wasm_bindgen]
pub fn build_argv(fade_in: f64, fade_out: f64, in_name: &str) -> Result<JsValue, JsValue> {
    let (argv, out_name) = gizza_ai_video_audio_fade_core::plan(in_name, fade_in, fade_out)
        .map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
