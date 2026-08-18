//! Browser-facing wasm-bindgen wrapper for /tools/churn-cohort-retention/.
//! The page passes every field as a string, so numbers/booleans are parsed here
//! and all validation stays in the shared core.
use wasm_bindgen::prelude::*;

fn truthy(v: &str, default: bool) -> bool {
    let s = v.trim().to_ascii_lowercase();
    if s.is_empty() {
        default
    } else {
        matches!(s.as_str(), "true" | "1" | "on" | "yes")
    }
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    data: &str,
    signups: &str,
    user: &str,
    date: &str,
    signup_date: &str,
    granularity: &str,
    periods: &str,
    metric: &str,
    values: &str,
    as_of: &str,
    header: &str,
    delimiter: &str,
    format: &str,
) -> Result<String, JsValue> {
    let periods = if periods.trim().is_empty() {
        6.0
    } else {
        periods
            .trim()
            .parse::<f64>()
            .map_err(|_| JsValue::from_str(&format!("periods must be a whole number, got '{periods}'")))?
    };
    gizza_ai_churn_cohort_retention_core::analyze(
        data,
        signups,
        user,
        date,
        signup_date,
        granularity,
        periods,
        metric,
        values,
        as_of,
        truthy(header, true),
        delimiter,
        format,
    )
    .map_err(|e| JsValue::from_str(&e))
}
