//! Browser-facing wasm-bindgen wrapper for /tools/video-to-animated-webp/.
//! Field order must match page/meta.toml: start, duration, fps, width, quality,
//! lossless, then the uploaded input filename.

use gizza_ai_block_utils::ArgvPlan;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn build_argv(
    start: f64,
    duration: f64,
    fps: f64,
    width: f64,
    quality: f64,
    lossless: &str,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    let lossless = matches!(lossless.trim(), "true" | "1" | "on" | "yes");
    let (argv, out_name) = gizza_ai_video_to_animated_webp_core::plan(
        in_name, start, duration, fps, width, quality, lossless,
    )
    .map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
