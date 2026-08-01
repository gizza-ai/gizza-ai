//! Browser-facing wasm-bindgen wrapper for /tools/rent-vs-buy/.
//!
//! Field order MUST match page/meta.toml: home_price, down_payment_percent,
//! mortgage_rate_percent, loan_term_years, monthly_rent, years,
//! home_appreciation_percent, rent_growth_percent, investment_return_percent,
//! property_tax_percent, home_insurance_percent, maintenance_percent, hoa_monthly,
//! buying_closing_percent, selling_cost_percent, currency, decimals. Numeric values
//! arrive as strings; blank numeric strings are treated as "unset" (the core applies
//! the documented default). Currency passes through as an optional string.
use gizza_ai_rent_vs_buy_core::Inputs;
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

/// Compute the rent-vs-buy result from the supplied fields, returning a pretty-printed
/// JSON object. Throws the error string on failure.
#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    home_price: &str,
    down_payment_percent: &str,
    mortgage_rate_percent: &str,
    loan_term_years: &str,
    monthly_rent: &str,
    years: &str,
    home_appreciation_percent: &str,
    rent_growth_percent: &str,
    investment_return_percent: &str,
    property_tax_percent: &str,
    home_insurance_percent: &str,
    maintenance_percent: &str,
    hoa_monthly: &str,
    buying_closing_percent: &str,
    selling_cost_percent: &str,
    currency: &str,
    decimals: &str,
) -> Result<String, JsValue> {
    let inputs = Inputs {
        home_price: parse_opt("home_price", home_price).map_err(js)?,
        down_payment_percent: parse_opt("down_payment_percent", down_payment_percent).map_err(js)?,
        mortgage_rate_percent: parse_opt("mortgage_rate_percent", mortgage_rate_percent)
            .map_err(js)?,
        loan_term_years: parse_opt("loan_term_years", loan_term_years).map_err(js)?,
        monthly_rent: parse_opt("monthly_rent", monthly_rent).map_err(js)?,
        years: parse_opt("years", years).map_err(js)?,
        home_appreciation_percent: parse_opt("home_appreciation_percent", home_appreciation_percent)
            .map_err(js)?,
        rent_growth_percent: parse_opt("rent_growth_percent", rent_growth_percent).map_err(js)?,
        investment_return_percent: parse_opt("investment_return_percent", investment_return_percent)
            .map_err(js)?,
        property_tax_percent: parse_opt("property_tax_percent", property_tax_percent).map_err(js)?,
        home_insurance_percent: parse_opt("home_insurance_percent", home_insurance_percent)
            .map_err(js)?,
        maintenance_percent: parse_opt("maintenance_percent", maintenance_percent).map_err(js)?,
        hoa_monthly: parse_opt("hoa_monthly", hoa_monthly).map_err(js)?,
        buying_closing_percent: parse_opt("buying_closing_percent", buying_closing_percent)
            .map_err(js)?,
        selling_cost_percent: parse_opt("selling_cost_percent", selling_cost_percent).map_err(js)?,
        currency: opt_str(currency),
        decimals: parse_opt("decimals", decimals).map_err(js)?,
    };
    gizza_ai_rent_vs_buy_core::compute_json(&inputs).map_err(js)
}

fn js(e: String) -> JsValue {
    JsValue::from_str(&e)
}
