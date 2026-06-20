//! Browser-facing wasm-bindgen wrapper for the standalone /tools/video-mute/
//! page. Builds the ffmpeg argv (pure, shared with the chat block via core).
//! No fields — only the uploaded file's in_name.
use gizza_ai_block_utils::ArgvPlan;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn build_argv(in_name: &str) -> Result<JsValue, JsValue> {
    let (argv, out_name) = gizza_ai_video_mute_core::build_argv(in_name);
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
