//! Browser-facing wasm-bindgen wrapper for the standalone /tools/image-crop/ page.
//! Builds the ffmpeg argv (pure, shared with the chat block via core); the JS
//! page driver runs it through the browser ffmpeg bridge.

use wasm_bindgen::prelude::*;

use gizza_ai_block_utils::ArgvPlan;
use gizza_ai_image_crop_core::plan_crop;

/// Returns `{ argv: string[], out_name }` or throws a JS error string. The page
/// driver calls `build_argv(...fieldArgs, in_name)`, with field args in the order
/// the page's `[[input]]` fields are declared: x, y, width, height.
#[wasm_bindgen]
pub fn build_argv(
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    if x < 0.0 || y < 0.0 || width < 0.0 || height < 0.0 {
        return Err(JsValue::from_str("x, y, width, height must be >= 0"));
    }
    let (argv, out_name) = plan_crop(
        in_name,
        x as u32,
        y as u32,
        width as u32,
        height as u32,
    )
    .map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
