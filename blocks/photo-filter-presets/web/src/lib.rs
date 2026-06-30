//! Browser-facing wasm-bindgen wrapper for /tools/photo-filter-presets/ (ffmpeg page).
//! Builds the ffmpeg argv (pure, shared with the chat block via core); returns
//! the shared block_utils::ArgvPlan so the page driver gets { argv, out_name }.
//!
//! Page field order (meta.toml) MUST match this param order: `preset` (a
//! `<select>`), then the file (`in_name`). `tool.js` calls
//! `build_argv(...fieldArgs, inName)`.
use gizza_ai_block_utils::ArgvPlan;
use gizza_ai_photo_filter_presets_core::plan_named;
use wasm_bindgen::prelude::*;

/// `preset` is one of sepia|vintage|warm|cool|noir|grayscale|vivid|invert|fade
/// (empty defaults to sepia). Returns `{ argv: string[], out_name }` or throws a
/// JS error string.
#[wasm_bindgen]
pub fn build_argv(preset: &str, in_name: &str) -> Result<JsValue, JsValue> {
    let (argv, out_name) =
        plan_named(in_name, Some(preset)).map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
