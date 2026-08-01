//! Browser-facing wasm-bindgen wrapper for /tools/salary-convert/.
//!
//! Field order MUST match page/meta.toml: amount, period, hours_per_week,
//! days_per_week, weeks_per_year, currency. Numeric values arrive as strings;
//! blank numeric strings fall back to the documented default (40 h/wk, 5 d/wk,
//! 52 wk/yr). A blank `amount` is a friendly error.
use wasm_bindgen::prelude::*;

/// Parse a numeric field, falling back to `default` when blank.
fn num(label: &str, s: &str, default: f64) -> Result<f64, String> {
    let t = s.trim();
    if t.is_empty() {
        return Ok(default);
    }
    t.parse::<f64>()
        .map_err(|_| format!("{label} must be a number (got '{t}')"))
}

/// Compute every salary figure and return a pretty-printed JSON object.
#[wasm_bindgen]
pub fn run(
    amount: &str,
    period: &str,
    hours_per_week: &str,
    days_per_week: &str,
    weeks_per_year: &str,
    currency: &str,
) -> Result<String, JsValue> {
    let amount_t = amount.trim();
    if amount_t.is_empty() {
        return Err(JsValue::from_str("enter a pay amount to convert"));
    }
    let amount = amount_t
        .replace([',', '$', '£', '€'], "")
        .parse::<f64>()
        .map_err(|_| JsValue::from_str(&format!("amount must be a number (got '{amount_t}')")))?;
    let period = {
        let p = period.trim();
        if p.is_empty() {
            "annual"
        } else {
            p
        }
    };
    let hours_per_week = num("hours_per_week", hours_per_week, 40.0).map_err(js)?;
    let days_per_week = num("days_per_week", days_per_week, 5.0).map_err(js)?;
    let weeks_per_year = num("weeks_per_year", weeks_per_year, 52.0).map_err(js)?;
    let currency = {
        let c = currency.trim();
        if c.is_empty() {
            "$"
        } else {
            c
        }
    };

    gizza_ai_salary_convert_core::convert_json(
        amount,
        period,
        hours_per_week,
        days_per_week,
        weeks_per_year,
        currency,
    )
    .map_err(js)
}

fn js(e: String) -> JsValue {
    JsValue::from_str(&e)
}
