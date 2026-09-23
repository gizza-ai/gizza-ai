//! Browser-facing wasm-bindgen wrapper for /tools/date-add-subtract/.
//! Compiled with wasm-pack for the standalone /tools/date-add-subtract/ page.
//!
//! Field order MUST match page/meta.toml: date, operation, years, months,
//! weeks, days, hours, minutes, seconds, skip_weekends, weekend_days, holidays. The page
//! marshals every field as a string, so the numeric ones are parsed to
//! `Option<f64>` here (blank stays `None`, i.e. "not supplied") and the
//! checkbox is parsed to a bool. A blank or relative `date` resolves against
//! the browser's local date (`Date`), since wasm32-unknown-unknown has no
//! std clock.
use chrono::NaiveDate;
use gizza_ai_date_add_subtract_core::{shift_json, Inputs};
use wasm_bindgen::prelude::*;

/// Parse an optional numeric field. Blank means "not supplied" so the core can
/// tell it apart from an explicit 0; anything non-numeric is a clear error.
fn opt_num(value: &str, field: &str) -> Result<Option<f64>, String> {
    let t = value.trim();
    if t.is_empty() {
        return Ok(None);
    }
    t.parse::<f64>()
        .map(Some)
        .map_err(|_| format!("{field} must be a number, got '{value}'"))
}

/// Checkboxes arrive as `true`/`false`, but deep links may also send
/// `1`/`on`/`yes`. Anything else (including blank) is off.
fn checkbox(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    )
}

/// Shift `date` by the supplied duration, returning pretty-printed JSON.
/// On a parse/validation error it throws the error string.
#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    date: &str,
    operation: &str,
    years: &str,
    months: &str,
    weeks: &str,
    days: &str,
    hours: &str,
    minutes: &str,
    seconds: &str,
    skip_weekends: &str,
    weekend_days: &str,
    holidays: &str,
) -> Result<String, JsValue> {
    let inputs = Inputs {
        date: date.to_string(),
        operation: match operation.trim() {
            "" => "add".to_string(),
            o => o.to_string(),
        },
        years: opt_num(years, "years").map_err(|e| JsValue::from_str(&e))?,
        months: opt_num(months, "months").map_err(|e| JsValue::from_str(&e))?,
        weeks: opt_num(weeks, "weeks").map_err(|e| JsValue::from_str(&e))?,
        days: opt_num(days, "days").map_err(|e| JsValue::from_str(&e))?,
        hours: opt_num(hours, "hours").map_err(|e| JsValue::from_str(&e))?,
        minutes: opt_num(minutes, "minutes").map_err(|e| JsValue::from_str(&e))?,
        seconds: opt_num(seconds, "seconds").map_err(|e| JsValue::from_str(&e))?,
        skip_weekends: Some(checkbox(skip_weekends)),
        weekend_days: weekend_days.to_string(),
        holidays: holidays.to_string(),
    };
    shift_json(&inputs, today_local()).map_err(|e| JsValue::from_str(&e))
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
