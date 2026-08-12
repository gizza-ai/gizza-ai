//! Browser-facing wasm-bindgen wrapper for /tools/edge-detection/ (ffmpeg page).
//! Builds the ffmpeg argv (pure, shared with the chat block via core); returns
//! the shared block_utils::ArgvPlan so the page driver gets { argv, out_name }.
//!
//! Page field order (meta.toml) MUST match this param order: `method` (a
//! `<select>`), `low`, `high`, `blur`, `invert` (a checkbox), `format` (a
//! `<select>`), then the file (`in_name`). `tool.js` calls
//! `build_argv(...fieldArgs, inName)`.
use gizza_ai_block_utils::ArgvPlan;
use gizza_ai_edge_detection_core::plan_named;
use wasm_bindgen::prelude::*;

/// `method` is `canny|sobel|colormix` (empty defaults to canny). `low`/`high`
/// are the Canny hysteresis thresholds as 0–1 fractions; the page prefills the
/// descriptor defaults and a CLEARED field arrives as 0, which is a valid
/// "detect everything" threshold. `blur` is the Gaussian pre-pass sigma in
/// pixels (0 = off). `invert` arrives from the checkbox as `"true"`/`"false"`.
/// `format` is `png|jpg|webp` (empty defaults to png). Returns
/// `{ argv: string[], out_name }` or throws a JS error string.
#[wasm_bindgen]
pub fn build_argv(
    method: &str,
    low: f64,
    high: f64,
    blur: f64,
    invert: &str,
    format: &str,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    let invert = matches!(invert.trim(), "true" | "1" | "on" | "yes");
    let (argv, out_name) = plan_named(in_name, Some(method), low, high, blur, invert, Some(format))
        .map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
