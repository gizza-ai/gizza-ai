//! Browser-facing wasm-bindgen wrapper for /tools/waveform-image/ (ffmpeg page).
//! Builds the ffmpeg argv (pure, shared with the chat block via core); the JS
//! page driver runs it through the browser ffmpeg bridge.
//!
//! Page field order (meta.toml) MUST match this param order: `width`,
//! `height`, `color`, `background`, `split_channels`, `scale`, then the file
//! (`in_name`). `tool.js` calls `build_argv(...fieldArgs, inName)`.

use wasm_bindgen::prelude::*;

use gizza_ai_block_utils::ArgvPlan;
use gizza_ai_waveform_image_core::plan_waveform_image;

/// `width`/`height` are the image size in pixels (empty page fields arrive as
/// `0.0` → the 1200×300 defaults). `color`/`background` are `#RGB`/`#RRGGBB`
/// hex strings (empty color → the default accent, empty background →
/// transparent). `split_channels` is the checkbox value string (positive
/// truthy) and `scale` is `lin|sqrt|cbrt|log` (empty → lin). Returns
/// `{ argv, out_name }` (out_name is always `out.png`) or throws.
#[wasm_bindgen]
pub fn build_argv(
    width: f64,
    height: f64,
    color: &str,
    background: &str,
    split_channels: &str,
    scale: &str,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    let split_on = matches!(split_channels, "true" | "1" | "on" | "yes");
    let (argv, out_name) =
        plan_waveform_image(in_name, width, height, color, background, split_on, scale)
            .map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
