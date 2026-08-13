//! Browser-facing wasm-bindgen wrapper for /tools/cartesian-to-polar-csv/.
//! The argument order mirrors `page/meta.toml`'s `[[input]]` order.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    csv: &str,
    direction: &str,
    x_column: &str,
    y_column: &str,
    angle_unit: &str,
    angle_range: &str,
    decimals: &str,
    delimiter: &str,
    has_header: &str,
    keep_columns: &str,
    output: &str,
) -> Result<String, JsValue> {
    let decimals = if decimals.trim().is_empty() {
        6
    } else {
        decimals
            .trim()
            .parse::<i64>()
            .map_err(|_| JsValue::from_str("decimals must be a whole number"))?
    };
    let truthy = |value: &str, default: bool| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            default
        } else {
            matches!(
                trimmed.to_ascii_lowercase().as_str(),
                "true" | "1" | "on" | "yes"
            )
        }
    };
    gizza_ai_cartesian_to_polar_csv_core::convert(
        csv,
        direction,
        x_column,
        y_column,
        angle_unit,
        angle_range,
        decimals,
        delimiter,
        truthy(has_header, true),
        truthy(keep_columns, true),
        output,
    )
    .map_err(|e| JsValue::from_str(&e))
}
