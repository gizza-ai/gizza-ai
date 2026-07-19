//! Browser-facing wasm-bindgen wrapper for /tools/image-film-grain/ (ffmpeg page).
//! Builds the ffmpeg argv (pure, shared with the chat block via core); returns
//! the shared block_utils::ArgvPlan so the page driver gets { argv, out_name }.
//!
//! Page field order (meta.toml) MUST match this param order: `amount`, then
//! `monochrome` (a checkbox — arrives as "true"/"false"), then `format` (a
//! `<select>`), then the file (`in_name`). `tool.js` calls
//! `build_argv(...fieldArgs, inName)`.
use gizza_ai_block_utils::ArgvPlan;
use gizza_ai_image_film_grain_core::plan_named;
use wasm_bindgen::prelude::*;

/// `amount` is 0–100 (0 = unchanged image; the page prefills the descriptor
/// default 20 and a CLEARED field arrives as 0). `monochrome` is a checkbox
/// string ("true"/"false"; empty defaults to true — luma-only neutral grain).
/// `format` is `keep|png|jpg|webp` (empty defaults to keep). Returns
/// `{ argv: string[], out_name }` or throws a JS error string.
#[wasm_bindgen]
pub fn build_argv(
    amount: f64,
    monochrome: &str,
    format: &str,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    let (argv, out_name) = plan_named(in_name, amount, Some(monochrome), Some(format))
        .map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
