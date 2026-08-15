//! Browser-facing wasm-bindgen wrapper for /tools/image-drop-shadow/ (ffmpeg
//! page). Builds the ffmpeg argv (pure, shared with the chat block via core);
//! returns the shared block_utils::ArgvPlan so the page driver gets
//! { argv, out_name }.
//!
//! Page field order (meta.toml) MUST match this param order: `offset_x`,
//! `offset_y`, `blur`, `color` (hybrid swatch + hex text), `opacity`,
//! `padding`, `background` (also a color field), `clip_to_original` (a
//! checkbox), `format` (a `<select>`), then the file (`in_name`).
//! `tool.js` calls `build_argv(...fieldArgs, inName)`.
use gizza_ai_block_utils::ArgvPlan;
use gizza_ai_image_drop_shadow_core::plan_named;
use wasm_bindgen::prelude::*;

/// `offset_x`/`offset_y` are px (positive = right/down; a CLEARED field
/// arrives as 0, i.e. a centered shadow). `blur` is a CSS-style radius in px
/// (0 = hard edge). `color` is hex or a name (empty defaults to black).
/// `opacity` is percent 0–100 (cleared = 0 = invisible). `padding` is px with
/// 0 meaning auto-fit. `background` is `transparent` (default) or a solid
/// color. `clip_to_original` arrives from the checkbox as `"true"`/`"false"`.
/// `format` is `png|webp|jpg` (empty defaults to png). Returns
/// `{ argv: string[], out_name }` or throws a JS error string.
#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn build_argv(
    offset_x: f64,
    offset_y: f64,
    blur: f64,
    color: &str,
    opacity: f64,
    padding: f64,
    background: &str,
    clip_to_original: &str,
    format: &str,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    // Positive-truthy: the page sends "true"/"false"; be liberal about the
    // other shapes a checkbox has historically arrived as.
    let clip = matches!(
        clip_to_original.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    );
    let (argv, out_name) = plan_named(
        in_name,
        offset_x,
        offset_y,
        blur,
        Some(color),
        opacity,
        padding,
        Some(background),
        clip,
        Some(format),
    )
    .map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
