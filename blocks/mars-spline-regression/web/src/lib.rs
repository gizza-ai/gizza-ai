//! Browser-facing wasm-bindgen wrapper for /tools/mars-spline-regression/.
use gizza_ai_mars_spline_regression_core::Options;
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

#[wasm_bindgen]
pub fn run(
    data: &str,
    target: &str,
    features: &str,
    max_terms: &str,
    max_degree: &str,
    penalty: &str,
    prune: &str,
    nprune: &str,
    minspan: &str,
    endspan: &str,
    thresh: &str,
    allow_linear: &str,
    predict: &str,
    header: &str,
    decimals: &str,
    format: &str,
) -> Result<String, JsValue> {
    let o = Options {
        target: if target.trim().is_empty() { "last".into() } else { target.into() },
        features: features.into(),
        max_terms: max_terms.trim().parse().unwrap_or(21),
        max_degree: max_degree.trim().parse().unwrap_or(1),
        penalty: penalty.trim().parse().unwrap_or(3.0),
        prune: truthy(prune),
        nprune: nprune.trim().parse().unwrap_or(0),
        minspan: minspan.trim().parse().unwrap_or(0),
        endspan: endspan.trim().parse().unwrap_or(0),
        thresh: thresh.trim().parse().unwrap_or(0.001),
        allow_linear: truthy(allow_linear),
        predict: predict.into(),
        header: if header.trim().is_empty() { "auto".into() } else { header.into() },
        decimals: decimals.trim().parse().unwrap_or(4),
        format: if format.trim().is_empty() { "text".into() } else { format.into() },
    };
    gizza_ai_mars_spline_regression_core::run(data, &o).map_err(|e| JsValue::from_str(&e))
}
