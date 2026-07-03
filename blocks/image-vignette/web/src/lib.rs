//! Browser-facing wasm-bindgen wrapper for /tools/image-vignette/ (ffmpeg page).
//! Builds the ffmpeg argv (pure, shared with the chat block via core); returns
//! the shared block_utils::ArgvPlan so the page driver gets { argv, out_name }.
//!
//! Page field order (meta.toml) MUST match this param order: `strength`, then
//! `mode` (a `<select>`), then `color` (hybrid swatch + hex text), then
//! `center_x`, then `center_y`, then `format` (a `<select>`), then the file
//! (`in_name`). `tool.js` calls `build_argv(...fieldArgs, inName)`.
use gizza_ai_block_utils::ArgvPlan;
use gizza_ai_image_vignette_core::plan_named;
use wasm_bindgen::prelude::*;

/// `strength` is 0–100 (0 = unchanged image; the page prefills the descriptor
/// default 40 and a CLEARED field arrives as 0). `mode` is `darken|lighten`
/// (empty defaults to darken). `color` is a name or hex (empty defaults to
/// black — the classic vignette; non-black tints require darken mode).
/// `center_x`/`center_y` are percent of the image size (50 = middle; a cleared
/// field arrives as 0 = the left/top edge). `format` is `keep|png|jpg|webp`
/// (empty defaults to keep). Returns `{ argv: string[], out_name }` or throws
/// a JS error string.
#[wasm_bindgen]
pub fn build_argv(
    strength: f64,
    mode: &str,
    color: &str,
    center_x: f64,
    center_y: f64,
    format: &str,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    let (argv, out_name) = plan_named(
        in_name,
        strength,
        Some(mode),
        center_x,
        center_y,
        Some(color),
        Some(format),
    )
    .map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
