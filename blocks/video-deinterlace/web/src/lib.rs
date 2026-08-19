//! Browser-facing wasm-bindgen wrapper for /tools/video-deinterlace/ (ffmpeg
//! page). Builds the ffmpeg argv (pure, shared with the chat block via core);
//! returns the shared block_utils::ArgvPlan so the page driver gets
//! { argv, out_name }.
//!
//! Page field order (meta.toml) MUST match this param order: `filter`, `mode`,
//! `field_order`, `apply_to`, then the file (`in_name`). `tool.js` calls
//! `build_argv(...fieldArgs, inName)`.
use gizza_ai_block_utils::ArgvPlan;
use gizza_ai_video_deinterlace_core::plan;
use wasm_bindgen::prelude::*;

/// `filter` is `bwdif|yadif` (empty → bwdif), `mode` is `frame|field` (empty →
/// frame, i.e. keep the frame rate; `field` doubles it), `field_order` is
/// `auto|tff|bff` (empty → auto) and `apply_to` is `all|flagged` (empty → all).
/// Returns `{ argv, out_name }` or throws a JS error string. The page passes
/// the four fields then the uploaded file's `in_name` (field order = param
/// order).
#[wasm_bindgen]
pub fn build_argv(
    filter: &str,
    mode: &str,
    field_order: &str,
    apply_to: &str,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    let (argv, out_name) = plan(filter, mode, field_order, apply_to, in_name)
        .map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
