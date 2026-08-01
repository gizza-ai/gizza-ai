//! Browser-facing wasm-bindgen wrapper for /tools/mortgage-calculator/.
//!
//! Field order MUST match page/meta.toml: home_price, down_payment, loan_years,
//! annual_interest_rate_percent, annual_property_tax, annual_insurance,
//! monthly_hoa, extra_monthly_payment, decimals. Each value arrives as a string;
//! blank numeric strings are treated as "unset" (the core applies the documented
//! default).
use gizza_ai_mortgage_calculator_core::Inputs;
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

/// Compute the mortgage result from the supplied fields, returning a
/// pretty-printed JSON object. Throws the error string on failure.
#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    home_price: &str,
    down_payment: &str,
    loan_years: &str,
    annual_interest_rate_percent: &str,
    annual_property_tax: &str,
    annual_insurance: &str,
    monthly_hoa: &str,
    extra_monthly_payment: &str,
    decimals: &str,
) -> Result<String, JsValue> {
    let inputs = Inputs {
        home_price: parse_opt("home_price", home_price).map_err(js)?,
        down_payment: parse_opt("down_payment", down_payment).map_err(js)?,
        loan_years: parse_opt("loan_years", loan_years).map_err(js)?,
        annual_interest_rate_percent: parse_opt(
            "annual_interest_rate_percent",
            annual_interest_rate_percent,
        )
        .map_err(js)?,
        annual_property_tax: parse_opt("annual_property_tax", annual_property_tax).map_err(js)?,
        annual_insurance: parse_opt("annual_insurance", annual_insurance).map_err(js)?,
        monthly_hoa: parse_opt("monthly_hoa", monthly_hoa).map_err(js)?,
        extra_monthly_payment: parse_opt("extra_monthly_payment", extra_monthly_payment)
            .map_err(js)?,
        decimals: parse_opt("decimals", decimals).map_err(js)?,
    };
    gizza_ai_mortgage_calculator_core::compute_json(&inputs).map_err(js)
}

fn js(e: String) -> JsValue {
    JsValue::from_str(&e)
}
