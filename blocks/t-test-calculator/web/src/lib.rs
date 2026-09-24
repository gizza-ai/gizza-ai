//! Browser-facing wasm-bindgen wrapper for /tools/t-test-calculator/.
//! The page driver hands every field over as a raw string, so the three numeric
//! fields are parsed here and the core owns all validation and clamping — one
//! shared code path for chat, CLI, and page.
use wasm_bindgen::prelude::*;

fn or_default(s: &str, fallback: &str) -> String {
    if s.trim().is_empty() {
        fallback.to_string()
    } else {
        s.trim().to_string()
    }
}

fn parse_f64(name: &str, s: &str, fallback: f64) -> Result<f64, JsValue> {
    let t = s.trim();
    if t.is_empty() {
        Ok(fallback)
    } else {
        t.parse::<f64>()
            .map_err(|_| JsValue::from_str(&format!("{name} must be a number, got `{t}`")))
    }
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    data: &str,
    test: &str,
    format: &str,
    delimiter: &str,
    header: &str,
    mu: &str,
    tails: &str,
    alpha: &str,
    decimals: &str,
    output: &str,
) -> Result<String, JsValue> {
    gizza_ai_t_test_calculator_core::run(
        data,
        &or_default(test, "auto"),
        &or_default(format, "auto"),
        &or_default(delimiter, "auto"),
        &or_default(header, "auto"),
        parse_f64("mu", mu, 0.0)?,
        &or_default(tails, "two"),
        parse_f64("alpha", alpha, 0.05)?,
        parse_f64("decimals", decimals, 4.0)?,
        &or_default(output, "summary"),
    )
    .map_err(|e| JsValue::from_str(&e))
}
