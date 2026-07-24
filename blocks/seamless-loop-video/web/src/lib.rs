//! Browser wasm wrapper for `/tools/seamless-loop-video/`.
//!
//! Page field order must match this signature: duration, crossfade, audio,
//! quality, then the uploaded file name.

use gizza_ai_block_utils::ArgvPlan;
use gizza_ai_seamless_loop_video_core::plan;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn build_argv(
    duration: f64,
    crossfade: f64,
    audio: &str,
    quality: &str,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    let crossfade = if crossfade == 0.0 { 1.0 } else { crossfade };
    let (argv, out_name) =
        plan(in_name, duration, crossfade, audio, quality).map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
