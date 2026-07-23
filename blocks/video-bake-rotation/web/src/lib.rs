//! Browser-facing wasm-bindgen wrapper for /tools/video-bake-rotation/ (ffmpeg page).
//! Builds the ffmpeg argv (pure, shared with the chat block via core); returns
//! the shared block_utils::ArgvPlan so the page driver gets { argv, out_name }.
use gizza_ai_block_utils::ArgvPlan;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn build_argv(in_name: &str) -> Result<JsValue, JsValue> {
    let (argv, out_name) = gizza_ai_video_bake_rotation_core::plan(in_name).map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name }).map_err(|e| JsValue::from_str(&e.to_string()))
}
