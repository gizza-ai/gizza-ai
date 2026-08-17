//! Browser-facing wasm-bindgen wrapper for /tools/peak-detector/.
//! The page passes every field as a string, in meta.toml order; blank means
//! "use the default", so an untouched form behaves like the CLI's defaults.
use wasm_bindgen::prelude::*;

/// Parse a non-negative whole-number field (blank = `fallback`).
fn whole(raw: &str, fallback: u64, field: &str) -> Result<u64, JsValue> {
    match raw.trim() {
        "" => Ok(fallback),
        t => t
            .parse::<f64>()
            .ok()
            .filter(|v| v.is_finite() && *v >= 0.0 && v.fract() == 0.0)
            .map(|v| v as u64)
            .ok_or_else(|| JsValue::from_str(&format!("{field} must be a whole number of 0 or more"))),
    }
}

/// Parse a numeric field (blank = `fallback`).
fn number(raw: &str, fallback: f64, field: &str) -> Result<f64, JsValue> {
    match raw.trim() {
        "" => Ok(fallback),
        t => t
            .parse::<f64>()
            .ok()
            .filter(|v| v.is_finite())
            .ok_or_else(|| JsValue::from_str(&format!("{field} must be a number"))),
    }
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    data: &str,
    mode: &str,
    separator: &str,
    smooth: &str,
    min_value: &str,
    max_value: &str,
    threshold: &str,
    min_distance: &str,
    min_prominence: &str,
    min_width: &str,
    rel_height: &str,
    max_peaks: &str,
    sort_by: &str,
    format: &str,
) -> Result<String, JsValue> {
    gizza_ai_peak_detector_core::run_with_options(
        data,
        mode,
        separator,
        whole(smooth, 0, "smooth")?,
        min_value,
        max_value,
        number(threshold, 0.0, "threshold")?,
        whole(min_distance, 0, "min_distance")?,
        number(min_prominence, 0.0, "min_prominence")?,
        number(min_width, 0.0, "min_width")?,
        number(rel_height, 0.5, "rel_height")?,
        whole(max_peaks, 0, "max_peaks")?,
        sort_by,
        format,
    )
    .map_err(|e| JsValue::from_str(&e))
}
