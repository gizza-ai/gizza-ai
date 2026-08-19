//! Browser-facing wasm-bindgen wrapper for /tools/stepwise-feature-selection/.
//! Field order MUST match meta.toml: data, target, direction, criterion,
//! alpha_enter, alpha_remove, force, labels, header, decimals.
use wasm_bindgen::prelude::*;

/// Parse a page field that may be blank, falling back to the descriptor default.
fn num(v: &str, fallback: f64) -> f64 {
    match v.trim().parse::<f64>() {
        Ok(n) if n.is_finite() => n,
        _ => fallback,
    }
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    data: &str,
    target: &str,
    direction: &str,
    criterion: &str,
    alpha_enter: &str,
    alpha_remove: &str,
    force: &str,
    labels: &str,
    header: &str,
    decimals: &str,
) -> Result<String, JsValue> {
    // default-false checkbox: only an explicit truthy value turns it on.
    let has_header = matches!(
        header.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    );
    let decimals = num(decimals, 4.0).clamp(0.0, 10.0) as u32;
    gizza_ai_stepwise_feature_selection_core::run(
        data,
        target,
        direction,
        criterion,
        num(alpha_enter, 0.05),
        num(alpha_remove, 0.10),
        force,
        labels,
        has_header,
        decimals,
    )
    .map_err(|e| JsValue::from_str(&e))
}
