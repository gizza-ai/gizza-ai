//! Browser-facing wasm-bindgen wrapper for /tools/budget-planner/.
//! Field order MUST match meta.toml: income, mode, split, expenses, currency.
use wasm_bindgen::prelude::*;

/// Build the budget plan and return the aligned plain-text report. On invalid
/// input it throws the error string. `income` arrives as an f64 number field;
/// empty select/text fields arrive as "" and take their documented defaults.
#[wasm_bindgen]
pub fn run(
    income: f64,
    mode: &str,
    split: &str,
    expenses: &str,
    currency: &str,
) -> Result<String, JsValue> {
    gizza_ai_budget_planner_core::plan_report(income, mode, split, expenses, currency)
        .map_err(|e| JsValue::from_str(&e))
}
