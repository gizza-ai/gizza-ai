//! Browser-facing wasm-bindgen wrapper for /tools/rent-affordability/.
//!
//! Field order MUST match page/meta.toml: income, income_period, income_type,
//! tax_rate_percent, rent_to_income_ratio, monthly_debts, max_dti_ratio,
//! currency, decimals. Numeric values arrive as strings; blank numeric strings
//! are treated as "unset" (the core applies the documented default). The two
//! enum fields and currency pass through as optional strings.
use gizza_ai_rent_affordability_core::Inputs;
use wasm_bindgen::prelude::*;

/// Parse an optional numeric field: blank/whitespace → `None`; otherwise parse,
/// erroring on garbage.
fn parse_opt(label: &str, s: &str) -> Result<Option<f64>, String> {
    let t = s.trim();
    if t.is_empty() {
        return Ok(None);
    }
    t.parse::<f64>()
        .map(Some)
        .map_err(|_| format!("{label} must be a number (got '{t}')"))
}

/// A blank string field → `None` (core applies the default); otherwise the value.
fn opt_str(s: &str) -> Option<String> {
    let t = s.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}

/// Compute the rent-affordability result from the supplied fields, returning a
/// pretty-printed JSON object. Throws the error string on failure.
#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    income: &str,
    income_period: &str,
    income_type: &str,
    tax_rate_percent: &str,
    rent_to_income_ratio: &str,
    monthly_debts: &str,
    max_dti_ratio: &str,
    currency: &str,
    decimals: &str,
) -> Result<String, JsValue> {
    let inputs = Inputs {
        income: parse_opt("income", income).map_err(js)?,
        income_period: opt_str(income_period),
        income_type: opt_str(income_type),
        tax_rate_percent: parse_opt("tax_rate_percent", tax_rate_percent).map_err(js)?,
        rent_to_income_ratio: parse_opt("rent_to_income_ratio", rent_to_income_ratio)
            .map_err(js)?,
        monthly_debts: parse_opt("monthly_debts", monthly_debts).map_err(js)?,
        max_dti_ratio: parse_opt("max_dti_ratio", max_dti_ratio).map_err(js)?,
        currency: opt_str(currency),
        decimals: parse_opt("decimals", decimals).map_err(js)?,
    };
    gizza_ai_rent_affordability_core::compute_json(&inputs).map_err(js)
}

fn js(e: String) -> JsValue {
    JsValue::from_str(&e)
}
