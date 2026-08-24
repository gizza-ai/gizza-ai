//! Browser-facing wasm-bindgen wrapper for /tools/percentile-rank-calculator/.
//! Field order MUST match meta.toml: data, values, method, decimals, include_stats.
//! The page marshals every field as a string, so the numeric/boolean ones are parsed here.
use gizza_ai_percentile_rank_calculator_core::{decimals_from, report, Method, Options};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    data: &str,
    values: &str,
    method: &str,
    decimals: &str,
    include_stats: &str,
) -> Result<String, JsValue> {
    let method = if method.trim().is_empty() {
        Method::Weak
    } else {
        Method::parse(method).map_err(|e| JsValue::from_str(&e))?
    };
    let dp = {
        let t = decimals.trim();
        if t.is_empty() {
            2
        } else {
            t.parse::<i64>().map_err(|_| {
                JsValue::from_str(&format!(
                    "decimals must be a whole number, got '{decimals}'"
                ))
            })?
        }
    };
    let decimals = decimals_from(dp).map_err(|e| JsValue::from_str(&e))?;
    // The page renders a default-true checkbox, so an empty value means "unset" → true.
    let include_stats = match include_stats.trim().to_ascii_lowercase().as_str() {
        "" => true,
        v => matches!(v, "true" | "1" | "on" | "yes"),
    };
    report(
        data,
        values,
        &Options {
            method,
            decimals,
            include_stats,
        },
    )
    .map_err(|e| JsValue::from_str(&e))
}
