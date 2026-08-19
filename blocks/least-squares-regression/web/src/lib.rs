//! Browser-facing wasm-bindgen wrapper for /tools/least-squares-regression/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    data: &str,
    y_values: &str,
    degree: i64,
    header: &str,
    intercept: &str,
    predict_x: &str,
    decimals: i64,
    format: &str,
) -> Result<String, JsValue> {
    let intercept = matches!(
        intercept.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    );
    gizza_ai_least_squares_regression_core::run(
        data, y_values, degree, header, intercept, predict_x, decimals, format,
    )
    .map_err(|e| JsValue::from_str(&e))
}
