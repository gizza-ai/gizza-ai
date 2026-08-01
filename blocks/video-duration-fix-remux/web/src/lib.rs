//! Browser-facing wasm-bindgen wrapper for /tools/video-duration-fix-remux/ (ffmpeg page).
//! Builds the ffmpeg argv (pure, shared with the chat block via core); returns
//! the shared block_utils::ArgvPlan so the page driver gets { argv, out_name }.
use gizza_ai_block_utils::ArgvPlan;
use wasm_bindgen::prelude::*;

/// Page-side entry point. Field order MUST match `page/meta.toml` (container,
/// faststart, regen_timestamps, then the driver appends `in_name`). Booleans
/// arrive as the strings "true"/"false" from the ffmpeg field marshaller.
#[wasm_bindgen]
pub fn build_argv(
    container: &str,
    faststart: &str,
    regen_timestamps: &str,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    let is_on = |v: &str| matches!(v, "true" | "1" | "on" | "yes");
    let (argv, out_name) = gizza_ai_video_duration_fix_remux_core::plan(
        container,
        is_on(faststart),
        is_on(regen_timestamps),
        in_name,
    )
    .map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
