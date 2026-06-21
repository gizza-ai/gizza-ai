//! date-diff core — pure compute, shared by the chat skill block and the web page.
//! No wafer/wasm-bindgen deps.
//!
//! Computes the duration between two dates/datetimes, broken down into a calendar
//! breakdown (years, months, days, hours, minutes, seconds) and as a flat total in
//! each unit (total weeks, days, hours, minutes, seconds). All math is in naive
//! (timezone-free) civil time — the inputs are interpreted as-is. The breakdown
//! handles variable month lengths and leap years correctly by stepping the
//! calendar, not by dividing seconds.

use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime, NaiveTime, Timelike};
use serde::Serialize;

/// Structured result of a date difference.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DateDiff {
    /// The earlier endpoint, normalized to `YYYY-MM-DD` or `YYYY-MM-DDTHH:MM:SS`.
    pub from: String,
    /// The later endpoint, normalized.
    pub to: String,
    /// True when the user supplied `start` after `end` (we still report a
    /// positive magnitude, but flag the direction).
    pub negative: bool,
    /// Calendar breakdown: largest-unit-first, each component is the remainder
    /// after the larger units are removed.
    pub years: i64,
    pub months: i64,
    pub days: i64,
    pub hours: i64,
    pub minutes: i64,
    pub seconds: i64,
    /// Flat totals — the whole span expressed in a single unit.
    pub total_weeks: i64,
    pub total_days: i64,
    pub total_hours: i64,
    pub total_minutes: i64,
    pub total_seconds: i64,
    /// Human-readable one-line summary of the calendar breakdown.
    pub summary: String,
}

/// Parse a flexible date or datetime string into a naive datetime.
///
/// Accepts (in order):
/// - RFC-3339 with offset/Z (`2024-01-02T03:04:05Z`, `...+02:00`) — offset is
///   dropped, the wall-clock value is kept.
/// - `YYYY-MM-DDTHH:MM:SS` / `YYYY-MM-DD HH:MM:SS`
/// - `YYYY-MM-DDTHH:MM` / `YYYY-MM-DD HH:MM`
/// - `YYYY-MM-DD`, `YYYY/MM/DD`, `MM/DD/YYYY`, `DD.MM.YYYY` (midnight)
fn parse_datetime(s: &str) -> Result<NaiveDateTime, String> {
    let t = s.trim();
    if t.is_empty() {
        return Err("empty date".into());
    }

    // RFC-3339 / ISO-8601 with timezone: keep the wall-clock, drop the offset.
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(t) {
        return Ok(dt.naive_local());
    }

    // Datetime forms with 'T' or space separator.
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

    // Date-only forms → midnight.
    for fmt in ["%Y-%m-%d", "%Y/%m/%d", "%m/%d/%Y", "%d.%m.%Y"] {
        if let Ok(d) = NaiveDate::parse_from_str(t, fmt) {
            return Ok(d.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap()));
        }
    }

    Err(format!(
        "could not parse '{t}' as a date — use e.g. 2024-01-31 or 2024-01-31T08:30:00"
    ))
}

/// Step `dt` forward by whole calendar months without overflowing day-of-month
/// (clamps the day to the target month's length, like most date libraries).
fn add_months(dt: NaiveDateTime, months: i64) -> NaiveDateTime {
    let mut y = dt.year() as i64;
    let mut m = dt.month() as i64 - 1 + months; // 0-based month
    y += m.div_euclid(12);
    m = m.rem_euclid(12);
    let month = (m + 1) as u32;
    let year = y as i32;
    let last = last_day_of_month(year, month);
    let day = dt.day().min(last);
    NaiveDate::from_ymd_opt(year, month, day)
        .unwrap()
        .and_time(dt.time())
}

fn last_day_of_month(year: i32, month: u32) -> u32 {
    let (ny, nm) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    let first_next = NaiveDate::from_ymd_opt(ny, nm, 1).unwrap();
    (first_next - Duration::days(1)).day()
}

/// Compute the difference between two date/datetime strings.
pub fn diff(start: &str, end: &str) -> Result<DateDiff, String> {
    let a = parse_datetime(start)?;
    let b = parse_datetime(end)?;

    let negative = a > b;
    let (lo, hi) = if a <= b { (a, b) } else { (b, a) };

    // Calendar breakdown: peel off years+months by stepping forward and checking
    // we don't overshoot — this respects variable month lengths.
    let mut months_total: i64 =
        (hi.year() as i64 - lo.year() as i64) * 12 + (hi.month() as i64 - lo.month() as i64);
    if add_months(lo, months_total) > hi {
        months_total -= 1;
    }
    let anchor = add_months(lo, months_total);
    let years = months_total / 12;
    let months = months_total % 12;

    // Remaining sub-month span as a plain duration.
    let rem = hi - anchor;
    let total_secs_rem = rem.num_seconds();
    let days = total_secs_rem / 86_400;
    let after_days = total_secs_rem % 86_400;
    let hours = after_days / 3_600;
    let minutes = (after_days % 3_600) / 60;
    let seconds = after_days % 60;

    // Flat totals over the whole span.
    let whole = hi - lo;
    let total_seconds = whole.num_seconds();
    let total_minutes = total_seconds / 60;
    let total_hours = total_seconds / 3_600;
    let total_days = total_seconds / 86_400;
    let total_weeks = total_days / 7;

    let summary = build_summary(years, months, days, hours, minutes, seconds, negative);

    Ok(DateDiff {
        from: fmt_dt(lo),
        to: fmt_dt(hi),
        negative,
        years,
        months,
        days,
        hours,
        minutes,
        seconds,
        total_weeks,
        total_days,
        total_hours,
        total_minutes,
        total_seconds,
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

fn build_summary(
    years: i64,
    months: i64,
    days: i64,
    hours: i64,
    minutes: i64,
    seconds: i64,
    negative: bool,
) -> String {
    let mut parts: Vec<String> = Vec::new();
    let push = |parts: &mut Vec<String>, n: i64, unit: &str| {
        if n != 0 {
            parts.push(format!("{n} {unit}{}", if n == 1 { "" } else { "s" }));
        }
    };
    push(&mut parts, years, "year");
    push(&mut parts, months, "month");
    push(&mut parts, days, "day");
    push(&mut parts, hours, "hour");
    push(&mut parts, minutes, "minute");
    push(&mut parts, seconds, "second");
    if parts.is_empty() {
        return "0 seconds (same instant)".into();
    }
    let body = if parts.len() == 1 {
        parts[0].clone()
    } else {
        let last = parts.pop().unwrap();
        format!("{} and {}", parts.join(", "), last)
    };
    if negative {
        format!("{body} (end is before start)")
    } else {
        body
    }
}

/// Convenience wrapper returning the structured result as pretty JSON — used by
/// the web page (single string output) and CLI.
pub fn diff_json(start: &str, end: &str) -> Result<String, String> {
    let d = diff(start, end)?;
    serde_json::to_string_pretty(&d).map_err(|e| format!("serialize failed: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whole_years() {
        let d = diff("2020-01-01", "2024-01-01").unwrap();
        assert_eq!(d.years, 4);
        assert_eq!(d.months, 0);
        assert_eq!(d.days, 0);
        // span includes leap days of 2020 and 2024-not-reached → 1461 days.
        assert_eq!(d.total_days, 1461);
    }

    #[test]
    fn years_months_days() {
        let d = diff("2020-01-15", "2022-03-20").unwrap();
        assert_eq!(d.years, 2);
        assert_eq!(d.months, 2);
        assert_eq!(d.days, 5);
        assert!(!d.negative);
    }

    #[test]
    fn month_length_clamping() {
        let d = diff("2021-01-31", "2021-03-31").unwrap();
        assert_eq!(d.years, 0);
        assert_eq!(d.months, 2);
        assert_eq!(d.days, 0);
    }

    #[test]
    fn leap_day_span() {
        let d = diff("2024-02-28", "2024-03-01").unwrap();
        assert_eq!(d.years, 0);
        assert_eq!(d.months, 0);
        assert_eq!(d.days, 2);
        assert_eq!(d.total_days, 2);
    }

    #[test]
    fn time_components() {
        let d = diff("2024-01-01T08:00:00", "2024-01-02T10:30:15").unwrap();
        assert_eq!(d.days, 1);
        assert_eq!(d.hours, 2);
        assert_eq!(d.minutes, 30);
        assert_eq!(d.seconds, 15);
        assert_eq!(d.total_hours, 26);
    }

    #[test]
    fn negative_direction_reported_positive() {
        let d = diff("2024-06-01", "2024-01-01").unwrap();
        assert!(d.negative);
        assert_eq!(d.months, 5);
        assert!(d.summary.contains("before start"));
    }

    #[test]
    fn same_instant() {
        let d = diff("2024-01-01T12:00:00", "2024-01-01T12:00:00").unwrap();
        assert_eq!(d.total_seconds, 0);
        assert_eq!(d.summary, "0 seconds (same instant)");
    }

    #[test]
    fn flat_totals() {
        let d = diff("2024-01-01", "2024-01-08").unwrap();
        assert_eq!(d.total_weeks, 1);
        assert_eq!(d.total_days, 7);
        assert_eq!(d.total_hours, 168);
        assert_eq!(d.total_minutes, 10_080);
        assert_eq!(d.total_seconds, 604_800);
    }

    #[test]
    fn parses_rfc3339_with_offset() {
        let d = diff("2024-01-01T00:00:00+00:00", "2024-01-01T02:00:00+00:00").unwrap();
        assert_eq!(d.hours, 2);
    }

    #[test]
    fn parses_slash_and_us_formats() {
        assert!(diff("2024/01/01", "2024/02/01").is_ok());
        let d = diff("01/01/2024", "02/01/2024").unwrap();
        assert_eq!(d.months, 1);
    }

    #[test]
    fn rejects_garbage() {
        assert!(diff("not-a-date", "2024-01-01").is_err());
        assert!(diff("2024-01-01", "").is_err());
    }

    #[test]
    fn json_output_has_fields() {
        let j = diff_json("2020-01-01", "2024-01-01").unwrap();
        assert!(j.contains("\"years\": 4"));
        assert!(j.contains("\"summary\""));
        assert!(j.contains("\"total_days\": 1461"));
    }

    #[test]
    fn summary_grammar_singular_plural() {
        let d = diff("2024-01-01", "2025-02-02").unwrap();
        assert_eq!(d.summary, "1 year, 1 month and 1 day");
    }
}
