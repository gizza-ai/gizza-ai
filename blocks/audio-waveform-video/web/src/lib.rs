//! Browser-facing wasm-bindgen wrapper for /tools/audio-waveform-video/ (ffmpeg
//! page). Builds the ffmpeg argv (pure, shared with the chat block via core);
//! the JS page driver runs it through the browser ffmpeg bridge.
//!
//! Page field order (meta.toml) MUST match this param order: `mode`, `width`,
//! `height`, `color`, `color2`, `background`, `scale`, `fps`, then the file
//! (`in_name`). `tool.js` calls `build_argv(...fieldArgs, inName)`. The output
//! name is fixed to `out.mp4` (audio in, video out).
use gizza_ai_block_utils::ArgvPlan;
use gizza_ai_audio_waveform_video_core::plan_audio_waveform_video;
use wasm_bindgen::prelude::*;

/// `mode` is mirror|bars|wave|points; `width`/`height`/`fps` are numbers (0 or
/// empty → defaults); `color`/`color2`/`background` are hex (empty color →
/// accent, empty color2 → solid, empty background → slate); `scale` is
/// lin|sqrt|cbrt|log. Returns `{ argv, out_name: "out.mp4" }` or throws a JS
/// error string.
#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn build_argv(
    mode: &str,
    width: f64,
    height: f64,
    color: &str,
    color2: &str,
    background: &str,
    scale: &str,
    fps: f64,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    let (argv, out_name) = plan_audio_waveform_video(
        in_name, "out.mp4", mode, width, height, color, color2, background, scale, fps,
    )
    .map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
