//! Browser-facing wasm-bindgen wrapper for /tools/bitcrush/ (ffmpeg page).
//! Builds the ffmpeg argv (pure, shared with the chat block via core); the JS
//! page driver runs it through the browser ffmpeg bridge.
//!
//! Page field order (meta.toml) MUST match this param order: `bits`,
//! `sample_rate_hz`, `mix`, `drive`, `output_gain`, `anti_alias`, `mode`,
//! `format`, then the file (`in_name`). `tool.js` calls
//! `build_argv(...fieldArgs, inName)`.

use gizza_ai_bitcrush_core::{
    plan_bitcrush, DEFAULT_ANTI_ALIAS, DEFAULT_BITS, DEFAULT_DRIVE, DEFAULT_MIX,
    DEFAULT_OUTPUT_GAIN, DEFAULT_SAMPLE_RATE_HZ,
};
use gizza_ai_block_utils::ArgvPlan;
use wasm_bindgen::prelude::*;

fn default_if_blank(v: f64, default: f64) -> f64 {
    if v == 0.0 {
        default
    } else {
        v
    }
}

/// Empty page number fields arrive as `0.0`; every control whose range includes
/// zero (`mix`, `anti_alias`) also has an explicit placeholder/default, so a
/// blank field uses the documented default. Non-blank out-of-range values are
/// rejected by core with a clear error string.
#[wasm_bindgen]
pub fn build_argv(
    bits: f64,
    sample_rate_hz: f64,
    mix: f64,
    drive: f64,
    output_gain: f64,
    anti_alias: f64,
    mode: &str,
    format: &str,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    let (argv, out_name) = plan_bitcrush(
        in_name,
        default_if_blank(bits, DEFAULT_BITS),
        default_if_blank(sample_rate_hz, DEFAULT_SAMPLE_RATE_HZ),
        default_if_blank(mix, DEFAULT_MIX),
        default_if_blank(drive, DEFAULT_DRIVE),
        default_if_blank(output_gain, DEFAULT_OUTPUT_GAIN),
        default_if_blank(anti_alias, DEFAULT_ANTI_ALIAS),
        mode,
        format,
    )
    .map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
