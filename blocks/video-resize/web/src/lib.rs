//! Browser-facing wasm-bindgen wrapper for /tools/video-resize/. Page passes the
//! field values then the uploaded file's in_name (field order in meta.toml MUST
//! equal this param order). width/height of 0 mean "auto" (preserve aspect).
use gizza_ai_block_utils::ArgvPlan;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn build_argv(width: f64, height: f64, in_name: &str) -> Result<JsValue, JsValue> {
    let w = if width > 0.0 { Some(width as u32) } else { None };
    let h = if height > 0.0 { Some(height as u32) } else { None };
    let (argv, out_name) = gizza_ai_video_resize_core::plan(in_name, w, h).map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
