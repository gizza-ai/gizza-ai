//! Browser-facing wasm-bindgen wrapper for /tools/age-calculator/.
//! Compiled with wasm-pack for the standalone /tools/age-calculator/ page.
//!
//! Field order MUST match meta.toml: birthdate, as_of. When `as_of` is left
//! blank the current local date comes from the browser (`Date`), since
//! wasm32-unknown-unknown has no std clock.
use chrono::NaiveDate;
use wasm_bindgen::prelude::*;

/// Compute the age from `birthdate` as of `as_of` (or today, if `as_of` is
/// blank), returning a pretty-printed JSON object. On a parse error it throws
/// the error string.
#[wasm_bindgen]
pub fn run(birthdate: &str, as_of: &str) -> Result<String, JsValue> {
    let today = today_local();
    gizza_ai_age_calculator_core::age_json(birthdate, as_of, today)
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
