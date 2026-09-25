//! Browser-facing wasm-bindgen wrapper for /tools/covariance-matrix-builder/.
//! Field order MUST match meta.toml: data, labels, delimiter, header, matrix,
//! denominator, weights, decimals, stats, format.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    data: &str,
    labels: &str,
    delimiter: &str,
    header: &str,
    matrix: &str,
    denominator: &str,
    weights: &str,
    decimals: &str,
    stats: &str,
    format: &str,
) -> Result<String, JsValue> {
    let d = decimals.trim();
    let d: f64 = if d.is_empty() {
        6.0
    } else {
        d.parse()
            .map_err(|_| JsValue::from_str("decimals must be a whole number between 0 and 12"))?
    };
    // default-true checkbox: the page sends "true"/"false"; an absent value in a
    // deep link keeps the default.
    let s = stats.trim();
    let show_stats = s.is_empty() || matches!(s.to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes");
    gizza_ai_covariance_matrix_builder_core::run(
        data,
        labels,
        delimiter,
        header,
        matrix,
        denominator,
        weights,
        d,
        show_stats,
        format,
    )
    .map_err(|e| JsValue::from_str(&e))
}
