//! Browser-facing wasm-bindgen wrapper for /tools/image-shrink-for-sharing/ (ffmpeg page).
//! Builds the ffmpeg argv (pure, shared with the chat block via core); the JS
//! page driver runs it through the browser ffmpeg bridge.
//!
//! Field order MUST match page/meta.toml: max_dimension, quality, format,
//! strip_metadata — then `in_name` is appended by site/tool-ffmpeg.js.
use gizza_ai_block_utils::ArgvPlan;
use gizza_ai_image_shrink_for_sharing_core::plan_shrink;
use wasm_bindgen::prelude::*;

/// `max_dimension`/`quality` arrive as JS numbers; `strip_metadata` arrives as
/// the checkbox string "true"/"false" (never as a JS bool — a "false" string is
/// truthy, so it must stay a `&str` and be parsed positively here). Returns
/// `{ argv, out_name }` or throws a JS error string.
#[wasm_bindgen]
pub fn build_argv(
    max_dimension: f64,
    quality: f64,
    format: &str,
    strip_metadata: &str,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    let max_dim = if max_dimension > 0.0 {
        max_dimension.round() as u32
    } else {
        0
    };
    let q = quality.round().clamp(1.0, 100.0) as u8;
    let strip = matches!(strip_metadata.trim(), "true" | "1" | "on" | "yes");
    let (argv, out_name) =
        plan_shrink(max_dim, q, format, strip, in_name).map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
