//! Browser-facing wasm-bindgen wrapper for /tools/loop-video/ (ffmpeg page).
//! Page field order (meta.toml) MUST match: count, duration, then the file (in_name).
use gizza_ai_block_utils::ArgvPlan;
use gizza_ai_loop_video_core::plan;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn build_argv(count: f64, duration: f64, in_name: &str) -> Result<JsValue, JsValue> {
    let c = if count >= 1.0 { count.round() as u32 } else { 2 };
    let d = if duration > 0.0 { duration } else { 0.0 };
    let (argv, out_name) = plan(in_name, c, d).map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
