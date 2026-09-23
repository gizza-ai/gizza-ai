//! date-add-subtract core — pure compute, shared by the chat skill block and the web page.
//! No wafer/wasm-bindgen deps.
//!
//! Shifts a date (or datetime) forward or backward by a duration expressed in
//! years, months, weeks, days, hours, minutes and seconds, and reports the
//! resulting date with the details people actually want next: the weekday, the
//! long-form date, the ISO week, the day of the year and how many calendar days
//! were crossed.
//!
//! Two counting modes:
//! - **Calendar** (default) — every day counts.
//! - **Business-day** (`skip_weekends` and/or a `holidays` list) — the day steps
//!   land only on working days; weekends and listed holidays are stepped over.
//!   Which days count as the weekend is configurable (`weekend_days`) so
//!   Friday–Saturday and Thursday–Friday working weeks are supported too.
//!
//! Order of operations is fixed and documented so results are reproducible:
//! years + months first (as calendar months, with the day-of-month clamped to
//! the target month's length), then the day steps (weeks × 7 + days), then the
//! time components as a plain duration. All math is naive (timezone-free) civil
//! time — inputs are interpreted as-is.

use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime, NaiveTime, Timelike, Weekday};
use serde::Serialize;

/// Hard cap on the number of working-day steps a single call may take (the
/// business-day walk is day-by-day, so this bounds the work). ≈ 800 years.
pub const MAX_BUSINESS_DAY_STEPS: i64 = 200_000;
/// Hard cap on how many holiday dates may be supplied.
pub const MAX_HOLIDAYS: usize = 1_000;

/// Per-unit magnitude caps. Anything larger is rejected with a clear message
/// rather than silently overflowing the calendar.
const CAP_YEARS: f64 = 10_000.0;
const CAP_MONTHS: f64 = 120_000.0;
const CAP_WEEKS: f64 = 520_000.0;
const CAP_DAYS: f64 = 3_650_000.0;
const CAP_HOURS: f64 = 87_600_000.0;
const CAP_MINUTES: f64 = 1_000_000_000.0;
const CAP_SECONDS: f64 = 1_000_000_000.0;

/// Everything the tool takes, as supplied by a surface (chat/CLI/page). Numeric
/// fields are `Option` so "not supplied" is distinct from an explicit 0.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Inputs {
    /// Start date or datetime. Blank (or `today`/`tomorrow`/`yesterday`) resolves
    /// against the `today` argument passed to [`shift`].
    pub date: String,
    /// `add` (default) or `subtract`.
    pub operation: String,
    pub years: Option<f64>,
    pub months: Option<f64>,
    pub weeks: Option<f64>,
    pub days: Option<f64>,
    pub hours: Option<f64>,
    pub minutes: Option<f64>,
    pub seconds: Option<f64>,
    /// Count only working days for the day steps.
    pub skip_weekends: Option<bool>,
    /// Which days form the weekend: `sat-sun` (default), `fri-sat`, `thu-fri`,
    /// `sun-only`, `fri-only` or `none`. Only consulted when `skip_weekends`.
    pub weekend_days: String,
    /// Comma/semicolon/newline-separated dates to treat as non-working days.
    pub holidays: String,
}

/// Structured result of a date shift.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DateShift {
    /// The start date, normalized to `YYYY-MM-DD` or `YYYY-MM-DDTHH:MM:SS`.
    pub input: String,
    /// `add` or `subtract` — the direction that was applied.
    pub operation: String,
    /// Human-readable offset that was applied, e.g. `90 days` or
    /// `1 year, 2 months and 10 business days`. `none` when every unit was 0.
    pub applied: String,
    /// True when weekends and/or holidays were stepped over.
    pub business_day_mode: bool,
    /// Which days were treated as non-working, e.g.
    /// `Saturday, Sunday + 2 holiday dates`. `null` in calendar mode.
    pub non_working_days: Option<String>,
    /// The resulting date, normalized (date only when the time is midnight).
    pub result: String,
    /// The resulting date as `YYYY-MM-DD` (always present).
    pub result_date: String,
    /// The resulting wall-clock time as `HH:MM:SS` (always present).
    pub result_time: String,
    /// Long form of the resulting date, e.g. `Thursday, 17 September 2026`.
    pub result_long: String,
    /// Weekday name of the result, e.g. `Thursday`.
    pub weekday: String,
    /// True when the result falls on a Saturday or Sunday.
    pub is_weekend: bool,
    /// Day of the year of the result, 1–366.
    pub day_of_year: u32,
    /// ISO-8601 week of the result, e.g. `2026-W38`.
    pub iso_week: String,
    /// Calendar quarter the result falls in, e.g. `Q3 2026`.
    pub quarter: String,
    /// True when the result's year is a leap year.
    pub is_leap_year: bool,
    /// Number of days in the result's month (28–31).
    pub days_in_result_month: u32,
    /// Signed number of calendar days between the input date and the result
    /// date (negative when the result is earlier).
    pub calendar_days_moved: i64,
    /// Signed number of working days stepped in business-day mode; `null` in
    /// calendar mode.
    pub business_days_moved: Option<i64>,
    /// How many weekend/holiday days were stepped over (0 in calendar mode).
    pub skipped_days: i64,
    /// The result as a Unix timestamp, interpreting the wall clock as UTC.
    pub result_unix: i64,
    /// Stage-by-stage working, in the order the units are applied: months (years
    /// folded in) → day steps → time components. One entry per stage that moved
    /// the clock, so a result can always be checked by hand.
    pub steps: Vec<String>,
    /// Human-readable one-line answer.
    pub summary: String,
}

/// Parse a flexible date or datetime string into a naive datetime.
///
/// Accepts (in order):
/// - the relative keywords `today`, `tomorrow`, `yesterday` (resolved against
///   `today`, at midnight)
/// - RFC-3339 with offset/Z (`2026-06-19T08:30:00Z`, `...+02:00`) — the offset is
///   dropped and the wall-clock value kept
/// - `YYYY-MM-DDTHH:MM[:SS]` / `YYYY-MM-DD HH:MM[:SS]`
/// - `YYYY-MM-DD`, `YYYY/MM/DD`, `MM/DD/YYYY`, `DD.MM.YYYY` (midnight)
/// - month-name forms: `June 19, 2026`, `19 June 2026`, `Jun 19 2026`
fn parse_datetime(s: &str, today: NaiveDate) -> Result<NaiveDateTime, String> {
    let t = s.trim();
    if t.is_empty() {
        return Ok(midnight(today));
    }

    match t.to_ascii_lowercase().as_str() {
        "today" | "now" => return Ok(midnight(today)),
        "tomorrow" => {
            return today
                .succ_opt()
                .map(midnight)
                .ok_or_else(|| "date is out of range".to_string())
        }
        "yesterday" => {
            return today
                .pred_opt()
                .map(midnight)
                .ok_or_else(|| "date is out of range".to_string())
        }
        _ => {}
    }

    // RFC-3339 / ISO-8601 with timezone: keep the wall clock, drop the offset.
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(t) {
        return Ok(dt.naive_local());
    }

    for fmt in [
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%dT%H:%M",
        "%Y-%m-%d %H:%M",
        "%Y/%m/%d %H:%M:%S",
        "%Y/%m/%d %H:%M",
    ] {
        if let Ok(dt) = NaiveDateTime::parse_from_str(t, fmt) {
            return Ok(dt);
        }
    }

    parse_date_only(t).map(midnight)
}

/// Parse the date-only forms (used for the start date and for holiday entries).
fn parse_date_only(t: &str) -> Result<NaiveDate, String> {
    for fmt in [
        "%Y-%m-%d",
        "%Y/%m/%d",
        "%m/%d/%Y",
        "%d.%m.%Y",
        "%B %d, %Y",
        "%B %d %Y",
        "%d %B %Y",
        "%b %d, %Y",
        "%b %d %Y",
        "%d %b %Y",
    ] {
        if let Ok(d) = NaiveDate::parse_from_str(t, fmt) {
            return Ok(d);
        }
    }
    Err(format!(
        "could not parse '{t}' as a date — use e.g. 2026-06-19, 19 June 2026, \
         06/19/2026 or 2026-06-19T08:30:00"
    ))
}

fn midnight(d: NaiveDate) -> NaiveDateTime {
    d.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
}

/// Step `dt` by whole calendar months, clamping the day-of-month to the target
/// month's length (Jan 31 + 1 month → Feb 28/29, like every date library).
fn add_months(dt: NaiveDateTime, months: i64) -> Result<NaiveDateTime, String> {
    let total = dt.year() as i64 * 12 + (dt.month() as i64 - 1) + months;
    let year = total.div_euclid(12);
    let month = total.rem_euclid(12) as u32 + 1;
    // chrono's calendar runs to ±262143; stop short of the edge so the
    // subsequent day/time steps can't wrap.
    if year < -262_142 || year > 262_142 {
        return Err("resulting date is out of range".into());
    }
    let year = year as i32;
    let day = dt.day().min(last_day_of_month(year, month)?);
    NaiveDate::from_ymd_opt(year, month, day)
        .map(|d| d.and_time(dt.time()))
        .ok_or_else(|| "resulting date is out of range".to_string())
}

fn last_day_of_month(year: i32, month: u32) -> Result<u32, String> {
    let (ny, nm) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    NaiveDate::from_ymd_opt(ny, nm, 1)
        .and_then(|d| d.pred_opt())
        .map(|d| d.day())
        .ok_or_else(|| "resulting date is out of range".to_string())
}

/// Read one numeric unit: absent → 0, must be a finite whole number within `cap`.
fn whole(label: &str, v: Option<f64>, cap: f64) -> Result<i64, String> {
    let x = v.unwrap_or(0.0);
    if !x.is_finite() {
        return Err(format!("{label} must be a finite number"));
    }
    if x.fract() != 0.0 {
        return Err(format!(
            "{label} must be a whole number of {label} (got {x}) — use a smaller \
             unit instead, e.g. 36 hours rather than 1.5 days"
        ));
    }
    if x.abs() > cap {
        return Err(format!(
            "{label} must be between {} and {} (got {x})",
            -(cap as i64),
            cap as i64
        ));
    }
    Ok(x as i64)
}

/// `add` (default) → +1, `subtract` → -1.
fn direction(operation: &str) -> Result<(&'static str, i64), String> {
    match operation.trim().to_ascii_lowercase().as_str() {
        "" | "add" | "plus" | "+" | "after" | "forward" => Ok(("add", 1)),
        "subtract" | "sub" | "minus" | "-" | "before" | "back" | "ago" => Ok(("subtract", -1)),
        other => Err(format!(
            "operation must be 'add' or 'subtract' (got '{other}')"
        )),
    }
}

/// Split a holiday list on commas, semicolons and newlines (not spaces — so
/// `4 July 2026` stays one entry) and parse each entry.
fn parse_holidays(s: &str) -> Result<Vec<NaiveDate>, String> {
    let mut out: Vec<NaiveDate> = Vec::new();
    for raw in s.split([',', ';', '\n', '\r']) {
        let t = raw.trim();
        if t.is_empty() {
            continue;
        }
        if out.len() >= MAX_HOLIDAYS {
            return Err(format!(
                "too many holidays — at most {MAX_HOLIDAYS} dates are supported"
            ));
        }
        let d = parse_date_only(t).map_err(|e| format!("holiday list: {e}"))?;
        if !out.contains(&d) {
            out.push(d);
        }
    }
    out.sort_unstable();
    Ok(out)
}

fn is_weekend(d: NaiveDate) -> bool {
    matches!(d.weekday(), Weekday::Sat | Weekday::Sun)
}

/// Which weekdays form the non-working weekend. Empty = none.
fn parse_weekend_days(s: &str) -> Result<Vec<Weekday>, String> {
    match s.trim().to_ascii_lowercase().replace('_', "-").as_str() {
        "" | "sat-sun" => Ok(vec![Weekday::Sat, Weekday::Sun]),
        "fri-sat" => Ok(vec![Weekday::Fri, Weekday::Sat]),
        "thu-fri" => Ok(vec![Weekday::Thu, Weekday::Fri]),
        "sun-only" => Ok(vec![Weekday::Sun]),
        "fri-only" => Ok(vec![Weekday::Fri]),
        "none" => Ok(Vec::new()),
        other => Err(format!(
            "weekend_days must be one of sat-sun, fri-sat, thu-fri, sun-only, \
             fri-only or none (got '{other}')"
        )),
    }
}

fn weekday_name(w: Weekday) -> &'static str {
    match w {
        Weekday::Mon => "Monday",
        Weekday::Tue => "Tuesday",
        Weekday::Wed => "Wednesday",
        Weekday::Thu => "Thursday",
        Weekday::Fri => "Friday",
        Weekday::Sat => "Saturday",
        Weekday::Sun => "Sunday",
    }
}

/// `+13` / `-4` — steps always carry their direction so the working reads back.
fn signed(n: i64) -> String {
    if n < 0 {
        n.to_string()
    } else {
        format!("+{n}")
    }
}

fn plural(n: i64, unit: &str) -> String {
    format!("{} {unit}{}", n.abs(), if n.abs() == 1 { "" } else { "s" })
}

/// Compute the shifted date. `today` supplies the clock for a blank/relative
/// `date` so the core stays deterministic and each surface brings its own clock.
pub fn shift(inputs: &Inputs, today: NaiveDate) -> Result<DateShift, String> {
    let start = parse_datetime(&inputs.date, today)?;
    let (operation, sign) = direction(&inputs.operation)?;

    let years = whole("years", inputs.years, CAP_YEARS)?;
    let months = whole("months", inputs.months, CAP_MONTHS)?;
    let weeks = whole("weeks", inputs.weeks, CAP_WEEKS)?;
    let days = whole("days", inputs.days, CAP_DAYS)?;
    let hours = whole("hours", inputs.hours, CAP_HOURS)?;
    let minutes = whole("minutes", inputs.minutes, CAP_MINUTES)?;
    let seconds = whole("seconds", inputs.seconds, CAP_SECONDS)?;

    let holidays = parse_holidays(&inputs.holidays)?;
    let weekend = parse_weekend_days(&inputs.weekend_days)?;
    let skip_weekends = inputs.skip_weekends.unwrap_or(false);
    let business_day_mode = skip_weekends || !holidays.is_empty();
    let mut steps: Vec<String> = Vec::new();

    // Step 1 — calendar months (years fold into months).
    let month_steps = sign
        .checked_mul(
            years
                .checked_mul(12)
                .and_then(|y| y.checked_add(months))
                .ok_or_else(|| "resulting date is out of range".to_string())?,
        )
        .ok_or_else(|| "resulting date is out of range".to_string())?;
    let mut cur = add_months(start, month_steps)?;
    if month_steps != 0 {
        steps.push(format!(
            "{} months (years folded in): {} → {}",
            signed(month_steps),
            fmt_dt(start),
            fmt_dt(cur)
        ));
    }
    let after_months = cur;

    // Step 2 — day steps: weeks × 7 + days, walked over working days only in
    // business-day mode.
    let day_steps = weeks
        .checked_mul(7)
        .and_then(|w| w.checked_add(days))
        .ok_or_else(|| "resulting date is out of range".to_string())?;
    let mut skipped_days = 0i64;
    let mut business_days_moved = None;

    if business_day_mode {
        let countable = |d: NaiveDate| {
            !(skip_weekends && weekend.contains(&d.weekday()))
                && holidays.binary_search(&d).is_err()
        };
        let signed_steps = sign * day_steps;
        if signed_steps.abs() > MAX_BUSINESS_DAY_STEPS {
            return Err(format!(
                "business-day mode supports at most {MAX_BUSINESS_DAY_STEPS} working-day \
                 steps per call (got {}) — turn off weekend/holiday skipping for spans \
                 that large",
                signed_steps.abs()
            ));
        }
        let step = if signed_steps >= 0 { 1 } else { -1 };
        let mut remaining = signed_steps.abs();
        while remaining > 0 {
            cur = cur
                .checked_add_signed(Duration::days(step))
                .ok_or_else(|| "resulting date is out of range".to_string())?;
            if countable(cur.date()) {
                remaining -= 1;
            } else {
                skipped_days += 1;
            }
        }
        // A pure year/month shift can still land on a non-working day: roll to
        // the nearest working day in the direction of travel.
        while !countable(cur.date()) {
            cur = cur
                .checked_add_signed(Duration::days(step))
                .ok_or_else(|| "resulting date is out of range".to_string())?;
            skipped_days += 1;
        }
        business_days_moved = Some(signed_steps);
    } else {
        cur = cur
            .checked_add_signed(Duration::days(sign * day_steps))
            .ok_or_else(|| "resulting date is out of range".to_string())?;
    }

    if cur != after_months {
        steps.push(format!(
            "{} {}: {} → {}",
            signed(sign * day_steps),
            if business_day_mode {
                "working-day steps"
            } else {
                "days"
            },
            fmt_dt(after_months),
            fmt_dt(cur)
        ));
    }
    let after_days = cur;

    // Step 3 — time components as a plain duration.
    let time_secs = hours
        .checked_mul(3_600)
        .and_then(|h| minutes.checked_mul(60).map(|m| h + m))
        .and_then(|hm| hm.checked_add(seconds))
        .ok_or_else(|| "resulting date is out of range".to_string())?;
    let end = cur
        .checked_add_signed(Duration::seconds(sign * time_secs))
        .ok_or_else(|| "resulting date is out of range".to_string())?;
    if time_secs != 0 {
        steps.push(format!(
            "{} seconds of clock time: {} → {}",
            signed(sign * time_secs),
            fmt_dt(after_days),
            fmt_dt(end)
        ));
    }

    let applied = describe_offset(
        years,
        months,
        weeks,
        days,
        hours,
        minutes,
        seconds,
        business_day_mode,
    );
    let result_date = end.date();
    let result_long = result_date.format("%A, %-d %B %Y").to_string();
    let result_time = end.format("%H:%M:%S").to_string();
    let summary = build_summary(&applied, &fmt_dt(start), operation, &result_long, end);

    // Which days the business-day walk treated as non-working, spelled out so a
    // reader can check the count by hand. `None` in plain calendar mode.
    let non_working_days = business_day_mode.then(|| {
        let mut parts: Vec<String> = Vec::new();
        if skip_weekends && !weekend.is_empty() {
            parts.push(
                weekend
                    .iter()
                    .map(|w| weekday_name(*w))
                    .collect::<Vec<_>>()
                    .join(", "),
            );
        }
        if !holidays.is_empty() {
            parts.push(plural(holidays.len() as i64, "holiday date"));
        }
        if parts.is_empty() {
            "none".to_string()
        } else {
            parts.join(" + ")
        }
    });
    let days_in_result_month = last_day_of_month(result_date.year(), result_date.month())?;
    let is_leap_year = last_day_of_month(result_date.year(), 2)? == 29;

    Ok(DateShift {
        input: fmt_dt(start),
        operation: operation.to_string(),
        applied,
        business_day_mode,
        non_working_days,
        result: fmt_dt(end),
        result_date: result_date.format("%Y-%m-%d").to_string(),
        result_time,
        result_long,
        weekday: result_date.format("%A").to_string(),
        is_weekend: is_weekend(result_date),
        day_of_year: result_date.ordinal(),
        iso_week: format!(
            "{}-W{:02}",
            result_date.iso_week().year(),
            result_date.iso_week().week()
        ),
        quarter: format!(
            "Q{} {}",
            (result_date.month() - 1) / 3 + 1,
            result_date.year()
        ),
        is_leap_year,
        days_in_result_month,
        calendar_days_moved: (result_date - start.date()).num_days(),
        business_days_moved,
        skipped_days,
        result_unix: end.and_utc().timestamp(),
        steps,
        summary,
    })
}

fn fmt_dt(dt: NaiveDateTime) -> String {
    if dt.hour() == 0 && dt.minute() == 0 && dt.second() == 0 {
        dt.format("%Y-%m-%d").to_string()
    } else {
        dt.format("%Y-%m-%dT%H:%M:%S").to_string()
    }
}

/// "1 year, 2 months and 10 business days" — only non-zero units, day units
/// labelled "business" when weekends/holidays are being skipped.
#[allow(clippy::too_many_arguments)]
fn describe_offset(
    years: i64,
    months: i64,
    weeks: i64,
    days: i64,
    hours: i64,
    minutes: i64,
    seconds: i64,
    business: bool,
) -> String {
    let mut parts: Vec<String> = Vec::new();
    let push = |parts: &mut Vec<String>, n: i64, unit: &str| {
        if n != 0 {
            parts.push(format!(
                "{} {unit}{}",
                n.abs() * n.signum(),
                if n.abs() == 1 { "" } else { "s" }
            ));
        }
    };
    push(&mut parts, years, "year");
    push(&mut parts, months, "month");
    if business {
        push(&mut parts, weeks, "business week");
        push(&mut parts, days, "business day");
    } else {
        push(&mut parts, weeks, "week");
        push(&mut parts, days, "day");
    }
    push(&mut parts, hours, "hour");
    push(&mut parts, minutes, "minute");
    push(&mut parts, seconds, "second");
    if parts.is_empty() {
        return "none".into();
    }
    if parts.len() == 1 {
        return parts.remove(0);
    }
    let last = parts.pop().unwrap();
    format!("{} and {}", parts.join(", "), last)
}

fn build_summary(
    applied: &str,
    input: &str,
    operation: &str,
    result_long: &str,
    end: NaiveDateTime,
) -> String {
    let clock = if end.hour() == 0 && end.minute() == 0 && end.second() == 0 {
        String::new()
    } else if end.second() == 0 {
        format!(" at {}", end.format("%H:%M"))
    } else {
        format!(" at {}", end.format("%H:%M:%S"))
    };
    if applied == "none" {
        return format!("No offset applied — {input} is {result_long}{clock}");
    }
    let word = if operation == "add" {
        "after"
    } else {
        "before"
    };
    format!("{applied} {word} {input} is {result_long}{clock}")
}

/// Convenience wrapper returning the structured result as pretty JSON — used by
/// the web page (single string output) and the CLI/chat surface.
pub fn shift_json(inputs: &Inputs, today: NaiveDate) -> Result<String, String> {
    let s = shift(inputs, today)?;
    serde_json::to_string_pretty(&s).map_err(|e| format!("serialize failed: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn today() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 9, 23).unwrap()
    }

    fn inputs(date: &str) -> Inputs {
        Inputs {
            date: date.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn adds_days_across_month_boundaries() {
        let mut i = inputs("2026-06-19");
        i.days = Some(90.0);
        let s = shift(&i, today()).unwrap();
        assert_eq!(s.result, "2026-09-17");
        assert_eq!(s.weekday, "Thursday");
        assert_eq!(s.result_long, "Thursday, 17 September 2026");
        assert_eq!(s.calendar_days_moved, 90);
        assert_eq!(s.day_of_year, 260);
        assert_eq!(s.iso_week, "2026-W38");
        assert!(!s.business_day_mode);
        assert_eq!(s.business_days_moved, None);
        assert_eq!(s.applied, "90 days");
        assert_eq!(
            s.summary,
            "90 days after 2026-06-19 is Thursday, 17 September 2026"
        );
    }

    #[test]
    fn subtracts_days() {
        let mut i = inputs("2026-09-17");
        i.operation = "subtract".into();
        i.days = Some(90.0);
        let s = shift(&i, today()).unwrap();
        assert_eq!(s.result, "2026-06-19");
        assert_eq!(s.operation, "subtract");
        assert_eq!(s.calendar_days_moved, -90);
        assert!(s
            .summary
            .contains("90 days before 2026-09-17 is Friday, 19 June 2026"));
    }

    #[test]
    fn month_end_is_clamped() {
        let mut i = inputs("2026-01-31");
        i.months = Some(1.0);
        assert_eq!(shift(&i, today()).unwrap().result, "2026-02-28");

        let mut i = inputs("2026-03-31");
        i.operation = "subtract".into();
        i.months = Some(1.0);
        assert_eq!(shift(&i, today()).unwrap().result, "2026-02-28");
    }

    #[test]
    fn leap_day_plus_one_year_clamps_to_feb_28() {
        let mut i = inputs("2024-02-29");
        i.years = Some(1.0);
        let s = shift(&i, today()).unwrap();
        assert_eq!(s.result, "2025-02-28");
        assert_eq!(s.applied, "1 year");
    }

    #[test]
    fn leap_year_is_crossed_correctly() {
        let mut i = inputs("2024-02-28");
        i.days = Some(2.0);
        assert_eq!(shift(&i, today()).unwrap().result, "2024-03-01");
    }

    #[test]
    fn mixed_units_apply_months_then_days_then_time() {
        let mut i = inputs("2026-01-31T10:00:00");
        i.years = Some(1.0);
        i.months = Some(1.0);
        i.weeks = Some(2.0);
        i.days = Some(3.0);
        i.hours = Some(5.0);
        i.minutes = Some(30.0);
        let s = shift(&i, today()).unwrap();
        // 2026-01-31 +13 months → 2027-02-28, +17 days → 2027-03-17, +5h30m.
        assert_eq!(s.result, "2027-03-17T15:30:00");
        assert_eq!(s.result_date, "2027-03-17");
        assert_eq!(s.result_time, "15:30:00");
        assert_eq!(
            s.applied,
            "1 year, 1 month, 2 weeks, 3 days, 5 hours and 30 minutes"
        );
        assert!(s.summary.ends_with("at 15:30"));
    }

    #[test]
    fn time_shift_can_roll_the_date() {
        let mut i = inputs("2026-06-19T22:00:00");
        i.hours = Some(5.0);
        let s = shift(&i, today()).unwrap();
        assert_eq!(s.result, "2026-06-20T03:00:00");
        assert_eq!(s.calendar_days_moved, 1);
    }

    #[test]
    fn negative_value_reverses_direction() {
        let mut i = inputs("2026-09-17");
        i.days = Some(-90.0);
        let s = shift(&i, today()).unwrap();
        assert_eq!(s.result, "2026-06-19");
        assert_eq!(s.applied, "-90 days");
    }

    #[test]
    fn business_days_skip_weekends() {
        let mut i = inputs("2026-06-19"); // Friday
        i.days = Some(1.0);
        i.skip_weekends = Some(true);
        let s = shift(&i, today()).unwrap();
        assert_eq!(s.result, "2026-06-22"); // Monday
        assert_eq!(s.weekday, "Monday");
        assert!(s.business_day_mode);
        assert_eq!(s.business_days_moved, Some(1));
        assert_eq!(s.skipped_days, 2);
        assert_eq!(s.calendar_days_moved, 3);
        assert_eq!(s.applied, "1 business day");
    }

    #[test]
    fn business_days_subtract() {
        let mut i = inputs("2026-06-22"); // Monday
        i.operation = "subtract".into();
        i.days = Some(5.0);
        i.skip_weekends = Some(true);
        let s = shift(&i, today()).unwrap();
        assert_eq!(s.result, "2026-06-15");
        assert_eq!(s.business_days_moved, Some(-5));
    }

    #[test]
    fn holidays_are_skipped() {
        let mut i = inputs("2026-07-01");
        i.days = Some(3.0);
        i.skip_weekends = Some(true);
        i.holidays = "2026-07-03, 4 July 2026".into();
        let s = shift(&i, today()).unwrap();
        assert_eq!(s.result, "2026-07-07");
        assert!(s.business_day_mode);
    }

    #[test]
    fn holidays_alone_enable_business_mode_without_skipping_weekends() {
        let mut i = inputs("2026-07-02"); // Thursday
        i.days = Some(2.0);
        i.holidays = "2026-07-03".into();
        let s = shift(&i, today()).unwrap();
        // Jul 3 is skipped, so 2 days lands on Jul 5 (a Sunday — weekends count).
        assert_eq!(s.result, "2026-07-05");
        assert!(s.is_weekend);
        assert_eq!(s.skipped_days, 1);
    }

    #[test]
    fn month_shift_rolls_off_a_weekend_in_business_mode() {
        let mut i = inputs("2026-04-30"); // Thursday
        i.months = Some(1.0);
        i.skip_weekends = Some(true);
        let s = shift(&i, today()).unwrap();
        // 2026-05-30 is a Saturday → rolls forward to Monday 2026-06-01.
        assert_eq!(s.result, "2026-06-01");
        assert_eq!(s.weekday, "Monday");
        assert_eq!(s.skipped_days, 2);
        assert_eq!(s.business_days_moved, Some(0));
    }

    #[test]
    fn blank_date_uses_today() {
        let mut i = inputs("");
        i.days = Some(1.0);
        let s = shift(&i, today()).unwrap();
        assert_eq!(s.input, "2026-09-23");
        assert_eq!(s.result, "2026-09-24");
    }

    #[test]
    fn relative_keywords_resolve_against_today() {
        let mut i = inputs("tomorrow");
        i.days = Some(0.0);
        assert_eq!(shift(&i, today()).unwrap().result, "2026-09-24");
        let mut i = inputs("YESTERDAY");
        i.weeks = Some(1.0);
        assert_eq!(shift(&i, today()).unwrap().result, "2026-09-29");
    }

    #[test]
    fn zero_offset_reports_no_change() {
        let s = shift(&inputs("2026-06-19"), today()).unwrap();
        assert_eq!(s.applied, "none");
        assert_eq!(s.result, "2026-06-19");
        assert_eq!(s.calendar_days_moved, 0);
        assert_eq!(
            s.summary,
            "No offset applied — 2026-06-19 is Friday, 19 June 2026"
        );
    }

    #[test]
    fn accepts_alternative_date_formats() {
        for (given, want) in [
            ("2026/06/19", "2026-06-19"),
            ("06/19/2026", "2026-06-19"),
            ("19.06.2026", "2026-06-19"),
            ("June 19, 2026", "2026-06-19"),
            ("19 June 2026", "2026-06-19"),
            ("Jun 19 2026", "2026-06-19"),
            ("2026-06-19T08:30:00Z", "2026-06-19T08:30:00"),
            ("2026-06-19 08:30", "2026-06-19T08:30:00"),
        ] {
            let s = shift(&inputs(given), today()).unwrap();
            assert_eq!(s.input, want, "parsing {given}");
        }
    }

    #[test]
    fn unix_timestamp_is_utc_wall_clock() {
        let s = shift(&inputs("1970-01-02T00:00:01"), today()).unwrap();
        assert_eq!(s.result_unix, 86_401);
    }

    #[test]
    fn rejects_unparseable_date() {
        let err = shift(&inputs("not-a-date"), today()).unwrap_err();
        assert!(err.contains("could not parse 'not-a-date'"), "{err}");
    }

    #[test]
    fn rejects_fractional_units() {
        let mut i = inputs("2026-06-19");
        i.months = Some(1.5);
        let err = shift(&i, today()).unwrap_err();
        assert!(err.contains("months must be a whole number"), "{err}");
    }

    #[test]
    fn rejects_unknown_operation() {
        let mut i = inputs("2026-06-19");
        i.operation = "multiply".into();
        let err = shift(&i, today()).unwrap_err();
        assert!(
            err.contains("operation must be 'add' or 'subtract'"),
            "{err}"
        );
    }

    #[test]
    fn rejects_out_of_cap_units() {
        let mut i = inputs("2026-06-19");
        i.years = Some(20_000.0);
        let err = shift(&i, today()).unwrap_err();
        assert!(
            err.contains("years must be between -10000 and 10000"),
            "{err}"
        );
    }

    #[test]
    fn rejects_bad_holiday_entry() {
        let mut i = inputs("2026-06-19");
        i.days = Some(1.0);
        i.holidays = "2026-07-04, nope".into();
        let err = shift(&i, today()).unwrap_err();
        assert!(
            err.contains("holiday list: could not parse 'nope'"),
            "{err}"
        );
    }

    #[test]
    fn rejects_oversized_business_day_walk() {
        let mut i = inputs("2026-06-19");
        i.days = Some(300_000.0);
        i.skip_weekends = Some(true);
        let err = shift(&i, today()).unwrap_err();
        assert!(err.contains("business-day mode supports at most"), "{err}");
    }

    #[test]
    fn accepts_the_documented_year_cap_boundary() {
        let mut i = inputs("2026-06-19");
        i.years = Some(10_000.0);
        let s = shift(&i, today()).unwrap();
        assert_eq!(s.result, "+12026-06-19");
    }

    #[test]
    fn json_output_has_fields() {
        let mut i = inputs("2026-06-19");
        i.days = Some(90.0);
        let j = shift_json(&i, today()).unwrap();
        assert!(j.contains("\"result\": \"2026-09-17\""), "{j}");
        assert!(j.contains("\"weekday\": \"Thursday\""), "{j}");
        assert!(j.contains("\"business_days_moved\": null"), "{j}");
        assert!(j.contains("\"summary\""), "{j}");
    }
}
