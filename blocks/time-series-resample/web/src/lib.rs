//! Browser-facing wasm-bindgen wrapper for /tools/time-series-resample/.
//! The page passes every field as a raw string (no coercion for pure tools);
//! a blank select/field means "use the default", which the core already
//! treats as such for every param here.
use gizza_ai_time_series_resample_core::resample;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    data: &str,
    interval: &str,
    aggregate: &str,
    time_column: &str,
    value_columns: &str,
    label: &str,
    closed: &str,
    fill: &str,
    origin: &str,
    offset: &str,
    time_format: &str,
    output: &str,
) -> Result<String, JsValue> {
    let interval = if interval.trim().is_empty() { "1h" } else { interval };
    resample(
        data,
        time_column,
        value_columns,
        interval,
        aggregate,
        label,
        closed,
        fill,
        origin,
        offset,
        time_format,
        output,
    )
    .map_err(|e| JsValue::from_str(&e))
}
