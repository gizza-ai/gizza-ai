//! Browser-facing wasm-bindgen wrapper for /tools/heart-rate-zones/.
//! Numeric params are `f64` (an `i64` would surface as a JS BigInt at runtime);
//! the page passes field values in `page/meta.toml` input order.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    age: f64,
    resting_hr: f64,
    max_hr: f64,
    max_hr_formula: &str,
    method: &str,
    model: &str,
    intensity: f64,
    output: &str,
) -> Result<String, JsValue> {
    gizza_ai_heart_rate_zones_core::run(
        age,
        resting_hr,
        max_hr,
        max_hr_formula,
        method,
        model,
        intensity,
        output,
    )
    .map_err(|e| JsValue::from_str(&e))
}
