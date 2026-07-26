//! Browser-facing wasm-bindgen wrapper for /tools/class-rebalancer/.
use wasm_bindgen::prelude::*;

fn truthy(v: &str, default: bool) -> bool {
    let s = v.trim().to_ascii_lowercase();
    if s.is_empty() {
        default
    } else {
        matches!(s.as_str(), "true" | "1" | "on" | "yes")
    }
}

fn parse_f64_default(v: &str, default: f64, name: &str) -> Result<f64, JsValue> {
    if v.trim().is_empty() {
        Ok(default)
    } else {
        v.trim()
            .parse::<f64>()
            .map_err(|_| JsValue::from_str(&format!("{name} must be a number")))
    }
}

fn parse_u64_default(v: &str, default: u64, name: &str) -> Result<u64, JsValue> {
    if v.trim().is_empty() {
        Ok(default)
    } else {
        v.trim()
            .parse::<u64>()
            .map_err(|_| JsValue::from_str(&format!("{name} must be a non-negative integer")))
    }
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    data: &str,
    label_column: &str,
    strategy: &str,
    target_ratio: &str,
    header: &str,
    shuffle: &str,
    seed: &str,
    output: &str,
) -> Result<String, JsValue> {
    gizza_ai_class_rebalancer_core::rebalance(
        data,
        label_column,
        if strategy.trim().is_empty() {
            "auto"
        } else {
            strategy
        },
        parse_f64_default(target_ratio, 1.0, "target_ratio")?,
        truthy(header, true),
        truthy(shuffle, false),
        parse_u64_default(seed, 42, "seed")?,
        if output.trim().is_empty() {
            "csv"
        } else {
            output
        },
    )
    .map_err(|e| JsValue::from_str(&e))
}
