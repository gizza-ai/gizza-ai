//! Browser-facing wasm-bindgen wrapper for /tools/calendar-freebusy-overlap/.
//! Field order MUST match page/meta.toml: calendar_a, calendar_b, start_date,
//! days, day_start, day_end, min_minutes, timezone, weekends, output.
use wasm_bindgen::prelude::*;

/// Find the free time windows common to both pasted .ics calendars.
///
/// The standalone tool page passes field values as strings (numbers coerce at
/// the wasm boundary): `days`/`min_minutes` arrive as f64 (0 = empty field →
/// default 7 / 30); `weekends` is the checkbox as `"true"`/`"false"`. The
/// page's clock supplies "today" when `start_date` is empty. Throws the error
/// string on invalid input.
#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    calendar_a: &str,
    calendar_b: &str,
    start_date: &str,
    days: f64,
    day_start: &str,
    day_end: &str,
    min_minutes: f64,
    timezone: &str,
    weekends: &str,
    output: &str,
) -> Result<String, JsValue> {
    let truthy = |v: &str| {
        matches!(
            v.trim().to_ascii_lowercase().as_str(),
            "true" | "1" | "yes" | "on"
        )
    };
    let days = if days == 0.0 { 7 } else { days as i64 };
    let min_minutes = if min_minutes == 0.0 { 30 } else { min_minutes as i64 };
    let now_utc_secs = (js_sys::Date::now() / 1000.0) as i64;
    gizza_ai_calendar_freebusy_overlap_core::run(
        calendar_a,
        calendar_b,
        start_date,
        days,
        day_start,
        day_end,
        min_minutes,
        timezone,
        truthy(weekends),
        output,
        now_utc_secs,
    )
    .map_err(|e| JsValue::from_str(&e))
}
