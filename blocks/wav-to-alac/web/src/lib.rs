//! Browser-facing wasm-bindgen wrapper for /tools/wav-to-alac/ (ffmpeg page).
//! Builds the ffmpeg argv (pure, shared with the chat block via core); the JS
//! page driver runs it through the browser ffmpeg bridge.
//!
//! Page field order (meta.toml) MUST match this param order: `bit_depth`,
//! `sample_rate`, `channels`, `keep_metadata`, then the file (`in_name`).
//! `tool.js` calls `build_argv(...fieldArgs, inName)`.
//!
//! All four selectors arrive as STRINGS: the driver only number-coerces params
//! whose declared schema type is numeric, and these are declared as string
//! enums / a boolean — so numeric-looking values like `"44100"` and `"16"` stay
//! strings, and the checkbox marshals as `"true"`/`"false"`.

use wasm_bindgen::prelude::*;

use gizza_ai_block_utils::ArgvPlan;
use gizza_ai_wav_to_alac_core::plan;

/// `bit_depth` is `source|16|24`, `sample_rate` is `source|44100|48000|88200|
/// 96000|176400|192000`, `channels` is `source|mono|stereo` (empty = `source`
/// for each), and `keep_metadata` is the checkbox state. `in_name` is the
/// uploaded file's name. Returns `{ argv, out_name }` or throws.
#[wasm_bindgen]
pub fn build_argv(
    bit_depth: &str,
    sample_rate: &str,
    channels: &str,
    keep_metadata: &str,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    // Positive-truthy: an unchecked box sends "false", a checked one "true".
    let keep = matches!(
        keep_metadata.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    );
    let (argv, out_name) =
        plan(in_name, bit_depth, sample_rate, channels, keep).map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
