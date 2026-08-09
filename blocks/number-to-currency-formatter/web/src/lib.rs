//! Browser-facing wasm-bindgen wrapper for /tools/number-to-currency-formatter/.
//! Field order MUST match meta.toml.
use gizza_ai_number_to_currency_formatter_core::format_currency;
use wasm_bindgen::prelude::*;

fn truthy(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

fn parse_decimals(raw: &str) -> Result<i64, JsValue> {
    match raw.trim() {
        "" => Ok(2),
        s => s.parse::<i64>().map_err(|_| {
            JsValue::from_str(&format!("decimals must be a whole number 0-8, got '{s}'"))
        }),
    }
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    value: &str,
    currency: &str,
    symbol_style: &str,
    position: &str,
    symbol_space: &str,
    decimals: &str,
    rounding: &str,
    grouping: &str,
    digit_grouping: &str,
    group_separator: &str,
    decimal_separator: &str,
    sign_style: &str,
    accounting: &str,
    trim_zeros: &str,
) -> Result<String, JsValue> {
    format_currency(
        value,
        if currency.trim().is_empty() {
            "USD"
        } else {
            currency
        },
        symbol_style,
        position,
        truthy(symbol_space),
        parse_decimals(decimals)?,
        rounding,
        if grouping.trim().is_empty() {
            true
        } else {
            truthy(grouping)
        },
        digit_grouping,
        group_separator,
        decimal_separator,
        sign_style,
        truthy(accounting),
        truthy(trim_zeros),
    )
    .map_err(|e| JsValue::from_str(&e))
}
