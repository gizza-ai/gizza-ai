//! Browser-facing wasm-bindgen wrapper for /tools/video-reverse/ (ffmpeg page).
//! Page field order (meta.toml) MUST match this param order: mode, audio,
//! quality, then file (`in_name`). tool.js calls build_argv(...fieldArgs, inName).
use gizza_ai_block_utils::ArgvPlan;
use gizza_ai_video_reverse_core::plan;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn build_argv(
    mode: &str,
    audio: &str,
    quality: &str,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    let (argv, out_name) =
        plan(in_name, mode, audio, quality).map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
