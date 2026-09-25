//! Browser-facing wasm-bindgen wrapper for /tools/aspect-ratio-calc/.
//! Field order MUST match meta.toml: ratio, width, height, rounding,
//! output_format. A blank number field arrives as NaN, which the core already
//! reads as "unknown, solve for it" — pass it straight through as 0.
use gizza_ai_aspect_ratio_calc_core::run as compute;
use wasm_bindgen::prelude::*;

/// An empty page number field arrives as NaN; both NaN and 0 mean "unknown".
fn dimension(v: f64) -> f64 {
    if v.is_nan() {
        0.0
    } else {
        v
    }
}

#[wasm_bindgen]
pub fn run(
    ratio: &str,
    width: f64,
    height: f64,
    rounding: &str,
    output_format: &str,
) -> Result<String, JsValue> {
    compute(
        ratio,
        dimension(width),
        dimension(height),
        rounding,
        output_format,
    )
    .map_err(|e| JsValue::from_str(&e))
}
