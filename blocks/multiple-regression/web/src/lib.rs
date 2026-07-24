//! Browser-facing wasm-bindgen wrapper for /tools/multiple-regression/.
//! Field order MUST match meta.toml: data, response, labels, intercept,
//! conf_level, format.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    data: &str,
    response: &str,
    labels: &str,
    intercept: &str,
    conf_level: &str,
    format: &str,
) -> Result<String, JsValue> {
    // default-true boolean: only an explicit false-y value turns it off.
    let has_intercept = !matches!(
        intercept.trim().to_ascii_lowercase().as_str(),
        "false" | "0" | "off" | "no"
    );
    let conf = conf_level.trim().parse::<f64>().unwrap_or(0.95);
    let fmt = if format.trim().is_empty() {
        "text"
    } else {
        format
    };
    let resp = if response.trim().is_empty() {
        "last"
    } else {
        response
    };
    gizza_ai_multiple_regression_core::run(data, resp, labels, has_intercept, conf, fmt)
        .map_err(|e| JsValue::from_str(&e))
}
