//! Browser-facing wasm-bindgen wrapper for /tools/video-rotate/. Field order in
//! meta.toml MUST equal this param order (rotate, flip, then the file in_name).
use gizza_ai_block_utils::ArgvPlan;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn build_argv(rotate: f64, flip: &str, in_name: &str) -> Result<JsValue, JsValue> {
    let rot = if rotate.is_finite() && rotate > 0.0 { rotate as u32 } else { 0 };
    let (argv, out_name) = gizza_ai_video_rotate_core::plan(in_name, rot, flip).map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
