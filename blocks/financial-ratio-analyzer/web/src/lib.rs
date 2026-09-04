//! Browser-facing wasm-bindgen wrapper for /tools/financial-ratio-analyzer/.
//! The page driver sends every value as a string; parse numeric and checkbox
//! fields here and let the shared core perform semantic validation.
use wasm_bindgen::prelude::*;

fn or_default(s: &str, fallback: &str) -> String {
    if s.trim().is_empty() {
        fallback.to_string()
    } else {
        s.trim().to_string()
    }
}

fn parse_i64(name: &str, s: &str, fallback: i64) -> Result<i64, JsValue> {
    let t = s.trim().replace([',', '_'], "");
    if t.is_empty() {
        Ok(fallback)
    } else {
        t.parse::<i64>().map_err(|_| {
            JsValue::from_str(&format!("{name} must be an integer, got `{}`", s.trim()))
        })
    }
}

fn parse_bool(s: &str, fallback: bool) -> bool {
    let t = s.trim().to_ascii_lowercase();
    if t.is_empty() {
        fallback
    } else {
        matches!(t.as_str(), "true" | "1" | "on" | "yes")
    }
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    figures: &str,
    prior_figures: &str,
    groups: &str,
    basis: &str,
    days_in_period: &str,
    benchmarks: &str,
    decimals: &str,
    currency: &str,
    output: &str,
) -> Result<String, JsValue> {
    gizza_ai_financial_ratio_analyzer_core::run(
        figures,
        prior_figures,
        &or_default(groups, "all"),
        &or_default(basis, "average"),
        parse_i64("days_in_period", days_in_period, 365)?,
        parse_bool(benchmarks, true),
        parse_i64("decimals", decimals, 2)?,
        currency,
        &or_default(output, "summary"),
    )
    .map_err(|e| JsValue::from_str(&e))
}
