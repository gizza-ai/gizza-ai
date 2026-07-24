//! Browser-facing wasm-bindgen wrapper for /tools/aiff-to-flac/ (ffmpeg page).
//! Builds the ffmpeg argv (pure, shared with the chat block via core); the JS
//! page driver runs it through the browser ffmpeg bridge.
//!
//! Page field order (meta.toml) MUST match this param order: `compression_level`,
//! then the file (`in_name`). `tool.js` calls `build_argv(...fieldArgs, inName)`.

use wasm_bindgen::prelude::*;

use gizza_ai_aiff_to_flac_core::{plan, DEFAULT_COMPRESSION_LEVEL};
use gizza_ai_block_utils::ArgvPlan;

/// `compression_level` is the FLAC level 0-12 (empty/0 keeps the default 5;
/// values above 12 clamp in core). `in_name` is the uploaded file's name.
/// Returns `{ argv, out_name }` or throws.
#[wasm_bindgen]
pub fn build_argv(compression_level: f64, in_name: &str) -> Result<JsValue, JsValue> {
    // An empty field arrives as 0.0; treat that as "use the default level" so a
    // blank control still produces a valid, well-compressed FLAC.
    let level = if compression_level > 0.0 {
        compression_level.round() as u32
    } else {
        DEFAULT_COMPRESSION_LEVEL
    };
    let (argv, out_name) = plan(in_name, level).map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
