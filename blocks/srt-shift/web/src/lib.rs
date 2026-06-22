//! Browser-facing wasm-bindgen wrapper for /tools/srt-shift/.
//! Compiled with wasm-pack for the standalone /tools/srt-shift/ page.
use wasm_bindgen::prelude::*;

/// Shift every timestamp in the SubRip text `srt` by `offset` (in `unit`).
///
/// The standalone tool page passes every field value as a string:
/// - `srt`: the full .srt subtitle text.
/// - `offset`: a (possibly fractional, possibly negative) number; blank → 0.
/// - `unit`: `"seconds"` (default/blank) or `"milliseconds"`.
///
/// Throws a JS error string on a bad unit / non-SRT input / malformed timing.
#[wasm_bindgen]
pub fn run(srt: &str, offset: &str, unit: &str) -> Result<String, JsValue> {
    let offset: f64 = offset.trim().parse().unwrap_or(0.0);
    let unit = unit.trim();
    let ms = match unit {
        "" | "seconds" => offset * 1000.0,
        "milliseconds" => offset,
        other => {
            return Err(JsValue::from_str(&format!(
                "invalid unit {other:?}: expected \"seconds\" or \"milliseconds\""
            )))
        }
    };
    gizza_ai_srt_shift_core::shift(srt, ms.round() as i64).map_err(|e| JsValue::from_str(&e))
}
