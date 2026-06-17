//! Browser-facing wasm-bindgen wrapper for /tools/image-grayscale/ (ffmpeg page).
//! Builds the ffmpeg argv (pure, shared with the chat block via core).
use serde::Serialize;
use wasm_bindgen::prelude::*;

#[derive(Serialize)]
struct Plan { argv: Vec<String>, out_name: String }

#[wasm_bindgen]
pub fn build_argv(in_name: &str) -> Result<JsValue, JsValue> {
    let (argv, out_name) = gizza_ai_image_grayscale_core::plan(in_name).map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&Plan { argv, out_name }).map_err(|e| JsValue::from_str(&e.to_string()))
}
