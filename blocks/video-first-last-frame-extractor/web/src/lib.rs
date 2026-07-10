//! Browser-facing wasm-bindgen wrapper for /tools/video-first-last-frame-extractor/
//! (ffmpeg page). Builds the ffmpeg argv (pure, shared with the chat block via
//! core); returns the shared block_utils::ArgvPlan so the page driver gets
//! { argv, out_name }.
//!
//! Page field order (page/meta.toml) MUST match this param order: `layout`,
//! `format`, then the file (`in_name`). `tool.js` calls
//! `build_argv(...fieldArgs, inName)`. All three params are strings.
use gizza_ai_block_utils::ArgvPlan;
use wasm_bindgen::prelude::*;

/// `layout` ∈ horizontal|vertical (how the first + last frame are joined);
/// `format` ∈ png|jpg. Returns `{ argv, out_name }` or throws the validation
/// error string.
#[wasm_bindgen]
pub fn build_argv(layout: &str, format: &str, in_name: &str) -> Result<JsValue, JsValue> {
    let (argv, out_name) =
        gizza_ai_video_first_last_frame_extractor_core::plan(in_name, layout, format)
            .map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
