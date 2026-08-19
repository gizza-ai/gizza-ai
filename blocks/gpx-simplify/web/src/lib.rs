//! Browser-facing wasm-bindgen wrapper for /tools/gpx-simplify/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    input: &str,
    tolerance_meters: &str,
    keep_every: &str,
    decimals: &str,
    output: &str,
) -> Result<String, JsValue> {
    let tolerance_meters = if tolerance_meters.trim().is_empty() {
        10.0
    } else {
        tolerance_meters
            .trim()
            .parse::<f64>()
            .map_err(|_| JsValue::from_str("tolerance_meters must be a number"))?
    };
    let keep_every = keep_every.trim().parse::<usize>().unwrap_or(0);
    let decimals = if decimals.trim().is_empty() {
        6
    } else {
        decimals
            .trim()
            .parse::<u32>()
            .map_err(|_| JsValue::from_str("decimals must be an integer"))?
    };
    gizza_ai_gpx_simplify_core::simplify(input, tolerance_meters, keep_every, decimals, output)
        .map_err(|e| JsValue::from_str(&e))
}
