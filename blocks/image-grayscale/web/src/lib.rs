//! Browser-facing wasm-bindgen wrapper for the standalone /tools/image-grayscale/
//! page. Builds the ffmpeg argv (pure, shared with the chat block via core); the
//! JS page driver runs it through the browser ffmpeg bridge.

use wasm_bindgen::prelude::*;

use gizza_ai_block_utils::ArgvPlan;
use gizza_ai_image_grayscale_core::plan;

/// Returns `{ argv: string[], out_name }` or throws a JS error string.
#[wasm_bindgen]
pub fn build_argv(in_name: &str) -> Result<JsValue, JsValue> {
    let (argv, out_name) = plan(in_name).map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
