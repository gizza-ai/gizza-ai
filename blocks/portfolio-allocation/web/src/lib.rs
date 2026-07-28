//! Browser-facing wasm-bindgen wrapper for /tools/portfolio-allocation/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    input: &str,
    group_by: &str,
    sort: &str,
    top_n: usize,
    currency: &str,
) -> Result<String, JsValue> {
    let group_by = gizza_ai_portfolio_allocation_core::parse_group_by(group_by)
        .map_err(|e| JsValue::from_str(&e))?;
    let sort =
        gizza_ai_portfolio_allocation_core::parse_sort(sort).map_err(|e| JsValue::from_str(&e))?;
    gizza_ai_portfolio_allocation_core::format_table(input, group_by, sort, top_n, currency)
        .map_err(|e| JsValue::from_str(&e))
}
