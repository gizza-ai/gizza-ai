//! Browser-facing wasm-bindgen wrapper for /tools/roas-calc/.
use wasm_bindgen::prelude::*;

/// Parse a numeric field, falling back to the descriptor default when blank.
fn number(value: &str, default: f64, label: &str) -> Result<f64, JsValue> {
    let v = value.trim();
    if v.is_empty() {
        return Ok(default);
    }
    v.parse::<f64>()
        .map_err(|_| JsValue::from_str(&format!("{label} must be a number")))
}

/// Checkboxes arrive as "true"/"false"; treat every positive spelling as on.
fn flag(value: &str, default: bool) -> bool {
    match value.trim().to_ascii_lowercase().as_str() {
        "" => default,
        "true" | "1" | "on" | "yes" => true,
        _ => false,
    }
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    ad_spend: &str,
    revenue: &str,
    conversions: &str,
    aov: &str,
    margin_basis: &str,
    gross_margin: &str,
    cogs_per_order: &str,
    shipping_per_order: &str,
    payment_rate: &str,
    payment_fixed: &str,
    refund_rate: &str,
    other_cost_per_order: &str,
    target_net_margin: &str,
    fixed_costs: &str,
    clicks: &str,
    cpc: &str,
    conversion_rate: &str,
    purchases_per_customer: &str,
    purchase_interval_months: &str,
    revenue_goal: &str,
    scenarios: &str,
    currency: &str,
    decimals: &str,
    format: &str,
) -> Result<String, JsValue> {
    gizza_ai_roas_calc_core::run(
        number(ad_spend, f64::NAN, "ad spend")?,
        number(revenue, 0.0, "revenue")?,
        number(conversions, 0.0, "conversions")?,
        number(aov, 0.0, "average order value")?,
        margin_basis,
        number(gross_margin, 50.0, "gross margin")?,
        number(cogs_per_order, 0.0, "cost of goods per order")?,
        number(shipping_per_order, 0.0, "shipping per order")?,
        number(payment_rate, 2.9, "payment rate")?,
        number(payment_fixed, 0.3, "payment fixed fee")?,
        number(refund_rate, 0.0, "refund rate")?,
        number(other_cost_per_order, 0.0, "other cost per order")?,
        number(target_net_margin, 0.0, "target net margin")?,
        number(fixed_costs, 0.0, "fixed costs")?,
        number(clicks, 0.0, "clicks")?,
        number(cpc, 0.0, "cost per click")?,
        number(conversion_rate, 0.0, "conversion rate")?,
        number(purchases_per_customer, 1.0, "purchases per customer")?,
        number(purchase_interval_months, 0.0, "purchase interval months")?,
        number(revenue_goal, 0.0, "revenue goal")?,
        flag(scenarios, true),
        currency,
        number(decimals, 2.0, "decimals")?,
        format,
    )
    .map_err(|e| JsValue::from_str(&e))
}
