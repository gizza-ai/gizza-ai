//! Browser-facing wasm-bindgen wrapper for /tools/round-to-nearest-multiple/.
//! The page passes every field value as a raw string, so parse them here and
//! funnel through the shared core (which owns all validation).
use gizza_ai_round_to_nearest_multiple_core::round_csv;
use wasm_bindgen::prelude::*;

fn truthy(v: &str) -> bool {
    matches!(v.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
pub fn run(
    data: &str,
    step: &str,
    mode: &str,
    columns: &str,
    header: &str,
    delimiter: &str,
    trailing_zeros: &str,
) -> Result<String, JsValue> {
    let step_val: f64 = if step.trim().is_empty() {
        1.0
    } else {
        step.trim()
            .parse()
            .map_err(|_| JsValue::from_str("step must be a number greater than 0"))?
    };
    let mode = if mode.is_empty() { "half_up" } else { mode };
    let delim = if delimiter.is_empty() { "," } else { delimiter };
    round_csv(data, step_val, mode, columns, truthy(header), delim, truthy(trailing_zeros))
        .map_err(|e| JsValue::from_str(&e))
}
