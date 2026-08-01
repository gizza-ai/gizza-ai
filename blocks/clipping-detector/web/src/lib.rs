//! Browser-facing wasm-bindgen wrapper for /tools/clipping-detector/.
//! Page fields arrive as strings; parse the numeric ones with defaults. The
//! core owns all validation (range checks, WAV decoding, error messages).
use wasm_bindgen::prelude::*;

fn parse_f64(s: &str, default: f64) -> f64 {
    let t = s.trim();
    if t.is_empty() {
        default
    } else {
        t.parse().unwrap_or(default)
    }
}

fn parse_u32(s: &str, default: u32) -> u32 {
    let t = s.trim();
    if t.is_empty() {
        default
    } else {
        t.parse().unwrap_or(default)
    }
}

/// Scan a WAV clip for clipped (full-scale) samples and report the worst regions.
///
/// - `input`: WAV file bytes as base64 or hex.
/// - `input_format`: `base64` (default) | `hex`.
/// - `output`: `report` (default) | `json`.
/// - `threshold`: fraction of full scale (0.5-1.0, default 0.99).
/// - `min_run` / `gap` / `top_regions`: integer knobs (empty → 1 / 0 / 5).
///
/// Throws a JS error string on invalid input or an out-of-range option.
#[wasm_bindgen]
pub fn run(
    input: &str,
    input_format: &str,
    output: &str,
    threshold: &str,
    min_run: &str,
    gap: &str,
    top_regions: &str,
) -> Result<String, JsValue> {
    gizza_ai_clipping_detector_core::run(
        input,
        input_format,
        output,
        parse_f64(threshold, 0.99),
        parse_u32(min_run, 1),
        parse_u32(gap, 0),
        parse_u32(top_regions, 5),
    )
    .map_err(|e| JsValue::from_str(&e))
}
