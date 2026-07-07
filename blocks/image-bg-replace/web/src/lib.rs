//! Browser-facing wasm-bindgen wrapper for /tools/image-bg-replace/ (ffmpeg page).
//! Builds the ffmpeg argv (pure, shared with the chat block via core); returns
//! the shared block_utils::ArgvPlan so the page driver gets { argv, out_name }.
//!
//! Page field order (meta.toml) MUST match this param order: `key_color`,
//! `similarity`, `blend`, `bg_type`, `bg_color`, `bg_color2`, `direction`,
//! `format`, then the file (`in_name`). `tool.js` calls
//! `build_argv(...fieldArgs, inName)`.
use gizza_ai_block_utils::ArgvPlan;
use gizza_ai_image_bg_replace_core::plan_named;
use wasm_bindgen::prelude::*;

/// `key_color` is the background color to remove (name or hex; empty → #00ff00
/// green). `similarity`/`blend` are 0-100 (a cleared field arrives as 0).
/// `bg_type` is `transparent|solid|gradient` (empty → solid). `bg_color` is the
/// solid fill / gradient start (empty → #ffffff); `bg_color2` is the gradient
/// end (empty → #000000). `direction` is `vertical|horizontal` (empty →
/// vertical). `format` is `png|webp|jpg|keep` (empty → png). Returns
/// `{ argv: string[], out_name }` or throws a JS error string.
#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn build_argv(
    key_color: &str,
    similarity: f64,
    blend: f64,
    bg_type: &str,
    bg_color: &str,
    bg_color2: &str,
    direction: &str,
    format: &str,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    let (argv, out_name) = plan_named(
        in_name,
        Some(key_color),
        similarity,
        blend,
        Some(bg_type),
        Some(bg_color),
        Some(bg_color2),
        Some(direction),
        Some(format),
    )
    .map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
