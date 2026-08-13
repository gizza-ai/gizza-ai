//! Browser-facing wasm-bindgen wrapper for /tools/principal-component-analysis/.
//! Field order MUST match meta.toml: data, labels, components, scale, format.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    data: &str,
    labels: &str,
    components: &str,
    scale: &str,
    format: &str,
) -> Result<String, JsValue> {
    let n = components.trim();
    let n: f64 = if n.is_empty() {
        0.0
    } else {
        n.parse()
            .map_err(|_| JsValue::from_str("components must be a whole number ≥ 0 (0 = keep every component)"))?
    };
    // default-true checkbox: only an explicit false-y value turns it off.
    let standardize = !matches!(
        scale.trim().to_ascii_lowercase().as_str(),
        "false" | "0" | "off" | "no"
    );
    let fmt = if format.trim().is_empty() {
        "text"
    } else {
        format
    };
    gizza_ai_principal_component_analysis_core::run(data, labels, n, standardize, fmt)
        .map_err(|e| JsValue::from_str(&e))
}
