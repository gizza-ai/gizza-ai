//! Browser-facing wasm-bindgen wrapper for /tools/portfolio-pnl/.
//! Param order here IS the page's `[[input]]` order in page/meta.toml.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    positions: &str,
    side: &str,
    fee_percent: f64,
    tax_rate: f64,
    sort: &str,
    currency: &str,
) -> Result<String, JsValue> {
    let side = gizza_ai_portfolio_pnl_core::Side::parse(side).map_err(|e| JsValue::from_str(&e))?;
    let sort = gizza_ai_portfolio_pnl_core::Sort::parse(sort).map_err(|e| JsValue::from_str(&e))?;
    let currency = if currency.is_empty() { "$" } else { currency };
    gizza_ai_portfolio_pnl_core::format_table(
        positions,
        side,
        fee_percent,
        tax_rate,
        sort,
        currency,
    )
    .map_err(|e| JsValue::from_str(&e))
}
