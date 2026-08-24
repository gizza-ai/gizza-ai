//! Browser-facing wasm-bindgen wrapper for /tools/image-median-denoise/ (ffmpeg
//! page). Builds the ffmpeg argv (pure, shared with the chat block via core);
//! returns the shared block_utils::ArgvPlan so the page driver gets
//! { argv, out_name }.
//!
//! Page field order (meta.toml) MUST match this param order: `radius`, then
//! `target` (a `<select>`), `channels` (a `<select>`), `passes`, `format`
//! (a `<select>`), `quality`, `strip_metadata` (a checkbox), then the file
//! (`in_name`). `tool.js` calls `build_argv(...fieldArgs, inName)`.
use gizza_ai_block_utils::ArgvPlan;
use gizza_ai_image_median_denoise_core::plan_named;
use wasm_bindgen::prelude::*;

/// `radius` is the median window radius in pixels, 1–20 (the window is
/// 2·radius+1 square); a CLEARED field arrives as 0 and falls back to the
/// descriptor default. `target` is `both|bright|dark`, `channels` is
/// `all|luma|chroma`, `format` is `keep|png|jpg|webp` (empty = the default in
/// each case). `passes` is 1–3 and `quality` is 1–100 for jpg/webp (0 = the
/// default). `strip_metadata` arrives from the checkbox as `"true"`/`"false"`.
/// Returns `{ argv: string[], out_name }` or throws a JS error string.
#[wasm_bindgen]
pub fn build_argv(
    radius: f64,
    target: &str,
    channels: &str,
    passes: f64,
    format: &str,
    quality: f64,
    strip_metadata: &str,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    let strip_metadata = matches!(strip_metadata.trim(), "true" | "1" | "on" | "yes");
    let (argv, out_name) = plan_named(
        in_name,
        radius,
        Some(target),
        Some(channels),
        passes,
        Some(format),
        quality,
        strip_metadata,
    )
    .map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
