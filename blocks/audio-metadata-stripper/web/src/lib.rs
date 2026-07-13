//! Browser-facing wasm-bindgen wrapper for /tools/audio-metadata-stripper/ (ffmpeg page).
//! Builds the ffmpeg argv (pure, shared with the chat block via core); returns
//! the shared block_utils::ArgvPlan so the page driver gets { argv, out_name }.
//!
//! Page field order (meta.toml) MUST match this param order: `cover_art`, then
//! the file (`in_name`). `tool.js` calls `build_argv(...fieldArgs, inName)`.

use gizza_ai_block_utils::ArgvPlan;
use wasm_bindgen::prelude::*;

/// `cover_art` is `remove|keep` (empty defaults to `remove` — a fully bare
/// copy). Returns `{ argv, out_name }` or throws a JS error string.
#[wasm_bindgen]
pub fn build_argv(cover_art: &str, in_name: &str) -> Result<JsValue, JsValue> {
    let (argv, out_name) =
        gizza_ai_audio_metadata_stripper_core::plan(in_name, cover_art).map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
