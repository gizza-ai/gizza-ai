//! Browser-facing wasm-bindgen wrapper for /tools/stl-inspector/.
//! Compiled with wasm-pack for the standalone /tools/stl-inspector/ page.
//!
//! Field order MUST match page/meta.toml: stl, input_format, output, units,
//! scale, density, weld_tolerance. The page passes every field value as a
//! string, so blanks fall back to the descriptor defaults here.
use gizza_ai_stl_inspector_core::{inspect, InputFormat, Options, Output, Units};
use wasm_bindgen::prelude::*;

/// Parse a numeric field, treating a blank/unparseable value as `fallback`.
fn num(s: &str, fallback: f64) -> f64 {
    let t = s.trim();
    if t.is_empty() {
        fallback
    } else {
        t.parse::<f64>().unwrap_or(fallback)
    }
}

/// Inspect an ASCII or binary STL and render a report or JSON.
///
/// - `stl`: ASCII STL text, or binary STL bytes as base64/hex.
/// - `input_format`: `"auto"` (default), `"ascii"`, `"base64"` or `"hex"`.
/// - `output`: `"report"` (default) or `"json"`.
/// - `units`: `"mm"` (default), `"cm"` or `"in"`.
/// - `scale`: coordinate multiplier applied before measuring (blank → 1).
/// - `density`: g/cm³ for the weight estimate (blank or 0 → omitted).
/// - `weld_tolerance`: vertex merge distance (blank → 0.000001).
///
/// Throws a JS error string on invalid arguments or an unparseable mesh.
#[wasm_bindgen]
pub fn run(
    stl: &str,
    input_format: &str,
    output: &str,
    units: &str,
    scale: &str,
    density: &str,
    weld_tolerance: &str,
) -> Result<String, JsValue> {
    let opt = Options {
        input_format: InputFormat::parse(input_format).map_err(|e| JsValue::from_str(&e))?,
        output: Output::parse(output).map_err(|e| JsValue::from_str(&e))?,
        units: Units::parse(units).map_err(|e| JsValue::from_str(&e))?,
        scale: num(scale, 1.0),
        density: num(density, 0.0),
        weld_tolerance: num(weld_tolerance, 0.000_001),
    };
    inspect(stl, &opt).map_err(|e| JsValue::from_str(&e))
}
