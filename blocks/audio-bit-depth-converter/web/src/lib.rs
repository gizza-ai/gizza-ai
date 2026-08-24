//! Browser-facing wasm-bindgen wrapper for /tools/audio-bit-depth-converter/
//! (ffmpeg page). Builds the ffmpeg argv (pure, shared with the chat block via
//! core); the JS page driver runs it through the browser ffmpeg bridge.
//!
//! Page field order (meta.toml) MUST match this param order: `bit_depth`,
//! `dither`, `format`, `keep_metadata`, then the file (`in_name`). `tool.js`
//! calls `build_argv(...fieldArgs, inName)` and passes every field as a STRING
//! (only params whose declared schema type is numeric get coerced — all four of
//! ours are enum/boolean, so `"16"` stays a string and never becomes `16`).

use wasm_bindgen::prelude::*;

use gizza_ai_audio_bit_depth_converter_core::{
    plan_convert, DEFAULT_DEPTH, DEFAULT_DITHER, DEFAULT_FORMAT,
};
use gizza_ai_block_utils::ArgvPlan;

/// Checkboxes marshal through `readField()` as `"true"`/`"false"`; parse
/// positive-truthy so an unchecked box (and only an unchecked box) strips tags.
fn truthy(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

/// Empty fields fall back to the same defaults the chat schema documents, so a
/// deep link that omits a param behaves exactly like a chat call that omits it.
fn or_default<'a>(v: &'a str, fallback: &'a str) -> &'a str {
    if v.trim().is_empty() {
        fallback
    } else {
        v
    }
}

/// `bit_depth` is `8|16|24|32f`, `dither` one of the 11 swresample methods
/// (`none` = truncate), `format` is `wav|flac`, `keep_metadata` is a checkbox
/// string. Returns `{ argv, out_name }` or throws the core's error message.
#[wasm_bindgen]
pub fn build_argv(
    bit_depth: &str,
    dither: &str,
    format: &str,
    keep_metadata: &str,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    let (argv, out_name) = plan_convert(
        in_name,
        or_default(bit_depth, DEFAULT_DEPTH),
        or_default(dither, DEFAULT_DITHER),
        or_default(format, DEFAULT_FORMAT),
        // A blank checkbox value means "not rendered / untouched" — keep the
        // documented default of preserving tags rather than silently stripping.
        keep_metadata.trim().is_empty() || truthy(keep_metadata),
    )
    .map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
