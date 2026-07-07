//! Browser-facing wasm-bindgen wrapper for /tools/image-resize-to-filesize/.
//!
//! Target-file-size needs a SEARCH, not a single ffmpeg pass, so `page/custom.js`
//! drives the loop: for each candidate quality it calls [`build_attempt`] to get
//! the argv, runs it through `ffmpegExec`, and measures the output — the exact
//! mirror of the chat/CLI block's `search_quality` loop, sharing the same `core`
//! argv builder + `Q_MIN`/`Q_MAX` bounds so nothing drifts.
//!
//! [`build_argv`] is only a defensive single-pass fallback for the (rare) case
//! where `custom.js` fails to import and the shared driver runs instead.

use gizza_ai_block_utils::ArgvPlan;
use gizza_ai_image_resize_to_filesize_core::{plan_attempt, Fmt};
use wasm_bindgen::prelude::*;

/// Fallback quality used only if the standard (non-custom) driver runs.
const FALLBACK_QUALITY: u8 = 70;

fn plan(fmt: &str, quality: u8, max_width: u32, in_name: &str) -> Result<ArgvPlan, JsValue> {
    let fmt = Fmt::from_arg(fmt).map_err(|e| JsValue::from_str(&e))?;
    let (argv, out_name) = plan_attempt(fmt, quality, max_width, in_name);
    Ok(ArgvPlan { argv, out_name })
}

/// Build the ffmpeg argv for ONE encode attempt at an explicit `quality`
/// (5-95). Called by `custom.js` once per binary-search step. Numeric params are
/// `f64` to avoid the wasm-bindgen BigInt path.
#[wasm_bindgen]
pub fn build_attempt(
    format: &str,
    quality: f64,
    max_width: f64,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    let q = quality.clamp(1.0, 100.0).round() as u8;
    let mw = max_width.max(0.0).round() as u32;
    let plan = plan(format, q, mw, in_name)?;
    serde_wasm_bindgen::to_value(&plan).map_err(|e| JsValue::from_str(&e.to_string()))
}

/// Defensive single-pass fallback (see module docs). Signature matches the page
/// field order (`target_kb`, `format`, `max_width`) so the shared driver can
/// still produce a valid image if `custom.js` doesn't load — it just encodes at
/// a fixed quality instead of searching the target.
#[wasm_bindgen]
pub fn build_argv(
    _target_kb: f64,
    format: &str,
    max_width: f64,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    let mw = max_width.max(0.0).round() as u32;
    let plan = plan(format, FALLBACK_QUALITY, mw, in_name)?;
    serde_wasm_bindgen::to_value(&plan).map_err(|e| JsValue::from_str(&e.to_string()))
}
