//! Browser-facing wasm-bindgen wrapper for /tools/waveform-image/ (ffmpeg page).
//! Builds the ffmpeg argv (pure, shared with the chat block via core); the JS
//! page driver runs it through the browser ffmpeg bridge.
//!
//! Page field order (meta.toml) MUST match this param order: `width`,
//! `height`, `color`, `color2`, `background`, `split_channels`, `scale`,
//! `sampling`, then the file (`in_name`). `tool.js` calls
//! `build_argv(...fieldArgs, inName)`.

use wasm_bindgen::prelude::*;

use gizza_ai_block_utils::ArgvPlan;
use gizza_ai_waveform_image_core::plan_waveform_image;

/// `width`/`height` are the image size in pixels (empty page fields arrive as
/// `0.0` → the 1200×300 defaults). `color` is a hex color (or comma-separated
/// per-channel list; empty → the default accent), `color2` an optional
/// gradient end color, `background` a hex (empty → transparent) — all accept
/// `#RGB`/`#RGBA`/`#RRGGBB`/`#RRGGBBAA`. `split_channels` is the checkbox
/// value string (positive truthy), `scale` is `lin|sqrt|cbrt|log` (empty →
/// lin) and `sampling` is `average|peak` (empty → average). Returns
/// `{ argv, out_name }` (out_name is always `out.png`) or throws.
#[wasm_bindgen]
#[allow(clippy::too_many_arguments)] // mirrors the page field order 1:1
pub fn build_argv(
    width: f64,
    height: f64,
    color: &str,
    color2: &str,
    background: &str,
    split_channels: &str,
    scale: &str,
    sampling: &str,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    let split_on = matches!(split_channels, "true" | "1" | "on" | "yes");
    let (argv, out_name) = plan_waveform_image(
        in_name, width, height, color, color2, background, split_on, scale, sampling,
    )
    .map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
