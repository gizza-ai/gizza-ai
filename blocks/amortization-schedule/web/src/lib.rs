//! Browser-facing wasm-bindgen wrapper for /tools/amortization-schedule/.
use wasm_bindgen::prelude::*;

fn parse_f64(name: &str, s: &str, fallback: f64) -> Result<f64, JsValue> {
    let t = s.trim().replace([',', '_', '%'], "");
    if t.is_empty() {
        Ok(fallback)
    } else {
        t.parse::<f64>()
            .map_err(|_| JsValue::from_str(&format!("{name} must be a number, got `{}`", s.trim())))
    }
}

fn or_default(s: &str, fallback: &str) -> String {
    if s.trim().is_empty() {
        fallback.to_string()
    } else {
        s.trim().to_string()
    }
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    loan_amount: &str,
    annual_interest_rate_percent: &str,
    loan_years: &str,
    loan_months: &str,
    payment_frequency: &str,
    start_date: &str,
    extra_payment: &str,
    extra_one_time: &str,
    extra_one_time_period: &str,
    schedule_view: &str,
    format: &str,
    currency_symbol: &str,
) -> Result<String, JsValue> {
    let inputs = gizza_ai_amortization_schedule_core::Inputs {
        loan_amount: Some(parse_f64("loan_amount", loan_amount, 300_000.0)?),
        annual_interest_rate_percent: Some(parse_f64(
            "annual_interest_rate_percent",
            annual_interest_rate_percent,
            6.0,
        )?),
        loan_years: Some(parse_f64("loan_years", loan_years, 30.0)?),
        loan_months: Some(parse_f64("loan_months", loan_months, 0.0)?),
        payment_frequency: Some(or_default(payment_frequency, "monthly")),
        start_date: Some(or_default(start_date, "2026-01-01")),
        extra_payment: Some(parse_f64("extra_payment", extra_payment, 0.0)?),
        extra_one_time: Some(parse_f64("extra_one_time", extra_one_time, 0.0)?),
        extra_one_time_period: Some(parse_f64(
            "extra_one_time_period",
            extra_one_time_period,
            1.0,
        )?),
        schedule_view: Some(or_default(schedule_view, "period")),
        format: Some(or_default(format, "table")),
        currency_symbol: Some(or_default(currency_symbol, "$")),
    };
    gizza_ai_amortization_schedule_core::run_inputs(&inputs).map_err(|e| JsValue::from_str(&e))
}
