//! Browser-facing wasm-bindgen wrapper for /tools/de-esser/ (ffmpeg page).
//! Builds the ffmpeg argv (pure, shared with the chat block via core); the JS
//! page driver runs it through the browser ffmpeg bridge.
//!
//! Page field order (meta.toml) MUST match this param order: `amount`, `band`,
//! `max_reduction`, `mode`, then `format`, then the file (`in_name`).
//! `tool.js` calls `build_argv(...fieldArgs, inName)`.

use wasm_bindgen::prelude::*;

use gizza_ai_block_utils::ArgvPlan;
use gizza_ai_de_esser_core::{plan_deess, DEFAULT_AMOUNT, DEFAULT_BAND, DEFAULT_MAX_REDUCTION};

/// The three numeric controls plus `mode` (`output|ess|input`, empty → output)
/// and `format` (`mp3|wav|ogg|flac|m4a`, empty → mp3). An empty page number
/// field arrives here as `0.0`; every control's valid range starts at 1, so
/// zero unambiguously means "use the default" and never a real setting.
/// Returns `{ argv, out_name }` or throws a JS error string.
#[wasm_bindgen]
pub fn build_argv(
    amount: f64,
    band: f64,
    max_reduction: f64,
    mode: &str,
    format: &str,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    let amount = if amount == 0.0 {
        DEFAULT_AMOUNT
    } else {
        amount
    };
    let band = if band == 0.0 { DEFAULT_BAND } else { band };
    let max_reduction = if max_reduction == 0.0 {
        DEFAULT_MAX_REDUCTION
    } else {
        max_reduction
    };
    let (argv, out_name) = plan_deess(in_name, amount, band, max_reduction, mode, format)
        .map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
