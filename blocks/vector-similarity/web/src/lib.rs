//! Browser-facing wasm-bindgen wrapper for /tools/vector-similarity/.
use wasm_bindgen::prelude::*;

fn parse_usize(name: &str, value: &str, default: usize) -> Result<usize, JsValue> {
    let v = value.trim();
    if v.is_empty() {
        return Ok(default);
    }
    v.parse::<usize>()
        .map_err(|_| JsValue::from_str(&format!("{name} must be a whole number")))
}

fn parse_f64(name: &str, value: &str, default: f64) -> Result<f64, JsValue> {
    let v = value.trim();
    if v.is_empty() {
        return Ok(default);
    }
    v.parse::<f64>()
        .map_err(|_| JsValue::from_str(&format!("{name} must be a number")))
}

fn truthy(value: &str, default: bool) -> bool {
    match value.trim().to_ascii_lowercase().as_str() {
        "" => default,
        "true" | "1" | "on" | "yes" => true,
        _ => false,
    }
}

#[wasm_bindgen]
pub fn run(
    query: &str,
    vectors: &str,
    metric: &str,
    top_k: &str,
    normalize: &str,
    hamming_tolerance: &str,
    decimals: &str,
    show_all_metrics: &str,
    output: &str,
) -> Result<String, JsValue> {
    gizza_ai_vector_similarity_core::run(
        query,
        vectors,
        metric,
        parse_usize("top_k", top_k, 5)?,
        truthy(normalize, false),
        parse_f64("hamming_tolerance", hamming_tolerance, 0.0)?,
        parse_usize("decimals", decimals, 6)?,
        truthy(show_all_metrics, true),
        output,
    )
    .map_err(|e| JsValue::from_str(&e))
}
