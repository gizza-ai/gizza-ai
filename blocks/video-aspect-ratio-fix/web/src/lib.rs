//! Browser-facing wasm-bindgen wrapper for /tools/video-aspect-ratio-fix/ (ffmpeg page).
//! Builds the ffmpeg argv (pure, shared with the chat block via core); returns
//! the shared block_utils::ArgvPlan so the page driver gets { argv, out_name }.
use gizza_ai_block_utils::ArgvPlan;
use wasm_bindgen::prelude::*;

/// Page-side entry point. Field order MUST match `page/meta.toml` (aspect,
/// custom_aspect, container, faststart, then the driver appends `in_name`).
/// Every field arrives as a string; booleans as "true"/"false".
#[wasm_bindgen]
pub fn build_argv(
    aspect: &str,
    custom_aspect: &str,
    container: &str,
    faststart: &str,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    let is_on = |v: &str| matches!(v, "true" | "1" | "on" | "yes");
    let (argv, out_name) = gizza_ai_video_aspect_ratio_fix_core::plan(
        aspect,
        custom_aspect,
        container,
        is_on(faststart),
        in_name,
    )
    .map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
