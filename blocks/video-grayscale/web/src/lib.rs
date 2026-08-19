//! Browser-facing wasm-bindgen wrapper for /tools/video-grayscale/ (ffmpeg
//! page). Builds the ffmpeg argv (pure, shared with the chat block via core);
//! the JS page driver runs it through the browser ffmpeg bridge.
//!
//! Page field order (meta.toml) MUST match this param order: `method`,
//! `intensity`, `tint`, `contrast`, `quality`, `keep_audio`, then the file
//! (`in_name`). `tool.js` calls `build_argv(...fieldArgs, inName)`.
use gizza_ai_block_utils::ArgvPlan;
use gizza_ai_video_grayscale_core::plan;
use wasm_bindgen::prelude::*;

/// `method`/`tint`/`quality` are the enum values (empty → the core defaults
/// bt709/none/balanced). `intensity` is 0–100 and passes through as-is — 0 is a
/// legitimate value there (it leaves the original colors). An empty `contrast`
/// field arrives as `0.0`, which is below the accepted 0.5 floor, so it is read
/// as the neutral `1.0` default. `keep_audio` is the checkbox value string
/// (positive truthy). Returns `{ argv, out_name }` or throws a JS error string.
#[wasm_bindgen]
pub fn build_argv(
    method: &str,
    intensity: f64,
    tint: &str,
    contrast: f64,
    quality: &str,
    keep_audio: &str,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    let contrast = if contrast == 0.0 { 1.0 } else { contrast };
    let keep = matches!(keep_audio, "true" | "1" | "on" | "yes");
    let (argv, out_name) = plan(in_name, method, intensity, tint, contrast, quality, keep)
        .map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
