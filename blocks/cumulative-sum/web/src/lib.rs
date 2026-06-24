//! Browser-facing wasm-bindgen wrapper for /tools/cumulative-sum/.
//! Compiled with wasm-pack for the standalone /tools/cumulative-sum/ page.
use wasm_bindgen::prelude::*;

/// True iff the page sent a truthy checkbox value. The page generator renders a
/// boolean param as a checkbox: a checked box sends `"true"`, unchecked `"false"`.
fn truthy(v: &str) -> bool {
    matches!(v, "true" | "1" | "on" | "yes")
}

/// Running total (prefix sum) of a list of numbers, with optional running
/// average, min, and max columns.
///
/// The standalone tool page passes every field value as a string:
/// - `numbers`: the values (separated by commas, spaces, or newlines).
/// - `average`, `min`, `max`: `"true"`/`"false"` from their checkboxes.
///
/// Throws a JS error string on a non-numeric value or an empty list.
#[wasm_bindgen]
pub fn run(numbers: &str, average: &str, min: &str, max: &str) -> Result<String, JsValue> {
    gizza_ai_cumulative_sum_core::cumulative(numbers, truthy(average), truthy(min), truthy(max))
        .map_err(|e| JsValue::from_str(&e))
}
