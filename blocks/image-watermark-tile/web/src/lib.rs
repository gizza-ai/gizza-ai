//! Browser-facing wasm-bindgen wrapper for /tools/image-watermark-tile/.
//!
//! Returns the ffmpeg argv plus the bundled font and the watermark text as extra
//! virtual-FS inputs, so the browser ffmpeg bridge can write them before exec —
//! the same files the chat/CLI surfaces write via `dispatch_ffmpeg_inputs`.
//! Numeric params arrive as `f64` (the wasm BigInt gotcha); non-finite values
//! fall back to the core's defaults.

use gizza_ai_block_utils::{encode_b64, ArgvPlanWithInputs};
use gizza_ai_image_watermark_tile_core::{
    plan_named, DEFAULT_ANGLE, DEFAULT_COLOR, DEFAULT_COLUMNS, DEFAULT_FONT_SIZE, DEFAULT_OPACITY,
    DEFAULT_ROWS, FONT_BYTES, FONT_FILE, TEXT_FILE,
};
use wasm_bindgen::prelude::*;

fn or_default(v: f64, fallback: f64) -> f64 {
    if v.is_finite() && v != 0.0 {
        v
    } else {
        fallback
    }
}

/// Checkboxes arrive as the strings "true"/"false" from the page driver.
fn truthy(v: &str) -> bool {
    matches!(v.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn build_argv(
    text: &str,
    font_size: f64,
    color: &str,
    opacity: f64,
    angle: f64,
    columns: f64,
    rows: f64,
    pattern: &str,
    outline: &str,
    format: &str,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    // angle 0 is a MEANINGFUL value (straight rows), so it must not be
    // defaulted away like the other numerics.
    let angle = if angle.is_finite() { angle } else { DEFAULT_ANGLE };
    let (argv, out_name) = plan_named(
        in_name,
        text,
        or_default(font_size, DEFAULT_FONT_SIZE),
        if color.trim().is_empty() { DEFAULT_COLOR } else { color },
        or_default(opacity, DEFAULT_OPACITY),
        angle,
        or_default(columns, DEFAULT_COLUMNS as f64),
        or_default(rows, DEFAULT_ROWS as f64),
        Some(pattern),
        truthy(outline),
        Some(format),
    )
    .map_err(|e| JsValue::from_str(&e))?;
    let inputs = vec![
        (FONT_FILE.to_string(), encode_b64(FONT_BYTES)),
        (TEXT_FILE.to_string(), encode_b64(text.as_bytes())),
    ];
    serde_wasm_bindgen::to_value(&ArgvPlanWithInputs { argv, out_name, inputs })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
