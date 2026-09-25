//! Browser-facing wasm-bindgen wrapper for /tools/npv-irr-calculator/.
//! The page driver hands every field over as a raw string, so the numeric
//! fields are parsed here and the core owns all validation — one shared code
//! path for chat, CLI, and page.
use wasm_bindgen::prelude::*;

fn or_default(s: &str, fallback: &str) -> String {
    if s.trim().is_empty() {
        fallback.to_string()
    } else {
        s.trim().to_string()
    }
}

fn parse_f64(name: &str, s: &str, fallback: f64) -> Result<f64, JsValue> {
    let t = s.trim().replace(['%', ',', '_'], "");
    if t.is_empty() {
        Ok(fallback)
    } else {
        t.parse::<f64>()
            .map_err(|_| JsValue::from_str(&format!("{name} must be a number, got `{}`", s.trim())))
    }
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    cash_flows: &str,
    initial_investment: &str,
    discount_rate: &str,
    period: &str,
    timing: &str,
    decimals: &str,
    currency: &str,
    output: &str,
) -> Result<String, JsValue> {
    let decimals = parse_f64("decimals", decimals, 2.0)?;
    gizza_ai_npv_irr_calculator_core::run(
        cash_flows,
        parse_f64("initial_investment", initial_investment, 0.0)?,
        parse_f64("discount_rate", discount_rate, 10.0)?,
        &or_default(period, "annual"),
        &or_default(timing, "end"),
        decimals.round() as i64,
        currency,
        &or_default(output, "summary"),
    )
    .map_err(|e| JsValue::from_str(&e))
}
