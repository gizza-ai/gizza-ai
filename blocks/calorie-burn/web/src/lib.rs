//! Browser-facing wasm-bindgen wrapper for /tools/calorie-burn/.
//! Numeric params are `f64` (an `i64` would surface as a JS BigInt at runtime);
//! the page passes field values in `page/meta.toml` input order.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    weight: f64,
    weight_unit: &str,
    duration: f64,
    duration_unit: &str,
    activity: &str,
    met: f64,
    basis: &str,
    output: &str,
) -> Result<String, JsValue> {
    gizza_ai_calorie_burn_core::run(
        weight,
        weight_unit,
        duration,
        duration_unit,
        activity,
        met,
        basis,
        output,
    )
    .map_err(|e| JsValue::from_str(&e))
}
