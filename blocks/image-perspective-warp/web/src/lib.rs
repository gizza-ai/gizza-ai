//! Browser-facing wasm-bindgen wrapper for /tools/image-perspective-warp/ (ffmpeg page).
//! Builds the ffmpeg argv (pure, shared with the chat block via core); returns
//! the shared block_utils::ArgvPlan so the page driver gets { argv, out_name }.
use gizza_ai_block_utils::ArgvPlan;
use gizza_ai_image_perspective_warp_core::{parse_interp, parse_mode, plan, Corners};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn build_argv(
    tl_x: &str,
    tl_y: &str,
    tr_x: &str,
    tr_y: &str,
    bl_x: &str,
    bl_y: &str,
    br_x: &str,
    br_y: &str,
    mode: &str,
    interpolation: &str,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    let corners: Corners<'_> = [tl_x, tl_y, tr_x, tr_y, bl_x, bl_y, br_x, br_y];
    let interp = parse_interp(Some(interpolation)).map_err(|e| JsValue::from_str(&e))?;
    let mode = parse_mode(Some(mode)).map_err(|e| JsValue::from_str(&e))?;
    let (argv, out_name) = plan(&corners, interp, mode, in_name).map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
