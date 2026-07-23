//! Browser-facing wasm-bindgen wrapper for /tools/debt-payoff/.
//! Compiled with wasm-pack for the standalone page.
//!
//! Field order MUST match meta.toml: debts, method, extra_payment, start_date.
//! The page passes every field value as a string, so `extra_payment` is parsed
//! here; when `start_date` is blank the current local date comes from the
//! browser (`Date`), since wasm32-unknown-unknown has no std clock. The core
//! owns all validation.
use chrono::NaiveDate;
use wasm_bindgen::prelude::*;

/// Build a debt-payoff plan and return it as pretty-printed JSON. On any invalid
/// input it throws the error string.
#[wasm_bindgen]
pub fn run(debts: &str, method: &str, extra_payment: &str, start_date: &str) -> Result<String, JsValue> {
    let extra: f64 = {
        let t = extra_payment.trim();
        if t.is_empty() {
            0.0
        } else {
            t.trim_start_matches('$')
                .replace(',', "")
                .parse()
                .map_err(|_| JsValue::from_str("extra monthly payment must be a number"))?
        }
    };
    gizza_ai_debt_payoff_core::plan_json(debts, method, extra, start_date, today_local())
        .map_err(|e| JsValue::from_str(&e))
}

/// The browser's current local date (year/month/day), as a `NaiveDate`.
fn today_local() -> NaiveDate {
    let now = js_sys::Date::new_0();
    let year = now.get_full_year() as i32;
    let month = now.get_month() + 1; // JS months are 0-based.
    let day = now.get_date();
    NaiveDate::from_ymd_opt(year, month, day)
        .unwrap_or_else(|| NaiveDate::from_ymd_opt(1970, 1, 1).unwrap())
}
