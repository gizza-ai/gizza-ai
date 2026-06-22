//! Browser-facing wasm-bindgen wrapper for the standalone /tools/video-crop/
//! page. Builds the ffmpeg argv (pure, shared with the chat block via core); the
//! JS page driver runs it through the browser ffmpeg bridge.
//!
//! The page passes the field values then the uploaded file's `in_name` (field
//! order in page/meta.toml MUST equal this param order). `x`/`y` of 0 mean
//! "unset" → centered crop.
use gizza_ai_block_utils::ArgvPlan;
use gizza_ai_video_crop_core::plan;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn build_argv(width: f64, height: f64, x: f64, y: f64, in_name: &str) -> Result<JsValue, JsValue> {
    let w = if width > 0.0 { width as u32 } else { 0 };
    let h = if height > 0.0 { height as u32 } else { 0 };
    let ox = if x > 0.0 { Some(x as u32) } else { None };
    let oy = if y > 0.0 { Some(y as u32) } else { None };
    let (argv, out_name) = plan(in_name, w, h, ox, oy).map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
