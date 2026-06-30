//! unix-timestamp-converter core — bidirectional conversion between Unix
//! (epoch) timestamps and human-readable dates, in many common formats.
//!
//! Pure Rust (`chrono`, alloc-only — no clock, no I/O). Two directions:
//!   * timestamp -> date: read a numeric epoch (in seconds / milliseconds /
//!     microseconds / nanoseconds, auto-detected by magnitude or forced via
//!     `unit`) and render the UTC date, ISO 8601 / RFC 2822 strings, and a full
//!     calendar breakdown.
//!   * date -> timestamp: read a date/time string in almost any common format
//!     (delegated to the `parse-datetime` core) and return the Unix timestamp in
//!     seconds, milliseconds, microseconds, and nanoseconds. A bare offset-less
//!     wall-clock is interpreted as UTC (flagged via `assumed_utc`).
//!
//! `mode = "auto"` (the default) picks the direction: a numeric token converts
//! timestamp -> date, anything else date -> timestamp.

use chrono::{DateTime, Datelike, NaiveDate, NaiveTime, TimeZone, Timelike, Utc};
use serde_json::{json, Value};

const MONTHS: [&str; 12] = [
    "January", "February", "March", "April", "May", "June", "July", "August",
    "September", "October", "November", "December",
];
const WEEKDAYS: [&str; 7] = [
    "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday",
];

/// Which way to convert.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// Decide from the input: numeric -> timestamp-to-date, else date-to-timestamp.
    Auto,
    /// Unix timestamp -> human-readable date.
    ToDate,
    /// Human-readable date -> Unix timestamp.
    ToTimestamp,
}

impl Direction {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "auto" => Ok(Direction::Auto),
            "to-date" | "to_date" | "todate" | "date" => Ok(Direction::ToDate),
            "to-timestamp" | "to_timestamp" | "totimestamp" | "timestamp" => {
                Ok(Direction::ToTimestamp)
            }
            other => Err(format!(
                "unknown mode '{other}' (use auto, to-date, or to-timestamp)"
            )),
        }
    }
}

/// The unit of a numeric timestamp.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    /// Detect from the integer magnitude.
    Auto,
    Seconds,
    Millis,
    Micros,
    Nanos,
}

impl Unit {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "auto" => Ok(Unit::Auto),
            "s" | "sec" | "secs" | "second" | "seconds" => Ok(Unit::Seconds),
            "ms" | "milli" | "millis" | "millisecond" | "milliseconds" => Ok(Unit::Millis),
            "us" | "µs" | "micro" | "micros" | "microsecond" | "microseconds" => Ok(Unit::Micros),
            "ns" | "nano" | "nanos" | "nanosecond" | "nanoseconds" => Ok(Unit::Nanos),
            other => Err(format!(
                "unknown unit '{other}' (use auto, seconds, milliseconds, microseconds, or nanoseconds)"
            )),
        }
    }
    fn label(self) -> &'static str {
        match self {
            Unit::Auto => "auto",
            Unit::Seconds => "seconds",
            Unit::Millis => "milliseconds",
            Unit::Micros => "microseconds",
            Unit::Nanos => "nanoseconds",
        }
    }
    /// Nanoseconds in one tick of this unit.
    fn nanos_per(self) -> i128 {
        match self {
            Unit::Auto | Unit::Seconds => 1_000_000_000,
            Unit::Millis => 1_000_000,
            Unit::Micros => 1_000,
            Unit::Nanos => 1,
        }
    }
}

/// Auto-detect the unit of an epoch from its integer magnitude. Thresholds are
/// the standard "digit count" heuristic: a present-day epoch is ~1.7e9 s,
/// ~1.7e12 ms, ~1.7e15 µs, ~1.7e18 ns.
fn detect_unit(int_magnitude: i128) -> Unit {
    if int_magnitude < 100_000_000_000 {
        Unit::Seconds // < 1e11
    } else if int_magnitude < 100_000_000_000_000 {
        Unit::Millis // < 1e14
    } else if int_magnitude < 100_000_000_000_000_000 {
        Unit::Micros // < 1e17
    } else {
        Unit::Nanos
    }
}

/// Is `s` a plain numeric token (optional sign, digits, optional single dot)?
fn is_numeric_token(s: &str) -> bool {
    let t = s.trim();
    let body = t.strip_prefix(['+', '-']).unwrap_or(t);
    if body.is_empty() {
        return false;
    }
    let mut seen_dot = false;
    let mut seen_digit = false;
    for c in body.chars() {
        match c {
            '.' if !seen_dot => seen_dot = true,
            '.' => return false,
            c if c.is_ascii_digit() => seen_digit = true,
            _ => return false,
        }
    }
    seen_digit
}

/// Convert a numeric timestamp token (with `unit`) to total nanoseconds since
/// the epoch, returning the resolved unit.
fn token_to_nanos(value: &str, unit: Unit) -> Result<(i128, Unit), String> {
    let t = value.trim();
    let neg = t.starts_with('-');
    let body = t.trim_start_matches(['+', '-']);
    let (int_part, frac_part) = body.split_once('.').unwrap_or((body, ""));

    let int_mag: i128 = if int_part.is_empty() {
        0
    } else {
        int_part
            .parse::<i128>()
            .map_err(|_| format!("timestamp '{value}' is out of range"))?
    };
    let resolved = match unit {
        Unit::Auto => detect_unit(int_mag),
        u => u,
    };
    let nanos_per = resolved.nanos_per();

    let mut total = int_mag
        .checked_mul(nanos_per)
        .ok_or_else(|| "timestamp out of range".to_string())?;
    if !frac_part.is_empty() {
        if !frac_part.bytes().all(|b| b.is_ascii_digit()) {
            return Err(format!("invalid timestamp '{value}'"));
        }
        total = total
            .checked_add(frac_to_nanos(frac_part, nanos_per))
            .ok_or_else(|| "timestamp out of range".to_string())?;
    }
    Ok((if neg { -total } else { total }, resolved))
}

/// `0.<frac>` of one tick (= `nanos_per` ns), rounded to the nearest nanosecond.
fn frac_to_nanos(frac: &str, nanos_per: i128) -> i128 {
    // Cap digits so 10^len stays well within i128.
    let digits: String = frac.chars().take(18).collect();
    let num: i128 = digits.parse().unwrap_or(0);
    let denom: i128 = 10i128.pow(digits.len() as u32);
    (num * nanos_per + denom / 2) / denom
}

/// Build a UTC datetime from total nanoseconds since the epoch.
fn datetime_from_nanos(total_nanos: i128) -> Result<DateTime<Utc>, String> {
    let secs = total_nanos.div_euclid(1_000_000_000);
    let nanos = total_nanos.rem_euclid(1_000_000_000) as u32;
    let secs_i64: i64 = secs
        .try_into()
        .map_err(|_| "timestamp is out of the representable date range".to_string())?;
    DateTime::from_timestamp(secs_i64, nanos)
        .ok_or_else(|| "timestamp is out of the representable date range".to_string())
}

/// Serialize a timestamp value as a JSON number when it fits serde_json's numeric
/// range, otherwise as a decimal string. This keeps normal epochs ergonomic while
/// still supporting forced-unit far-future/far-past dates without panicking.
fn i128_json(n: i128) -> Value {
    match i64::try_from(n) {
        Ok(v) => json!(v),
        Err(_) => json!(n.to_string()),
    }
}

/// The timestamp expressed in every unit (floored to whole units of each).
fn timestamps_value(total_nanos: i128) -> Value {
    json!({
        "seconds": i128_json(total_nanos.div_euclid(1_000_000_000)),
        "milliseconds": i128_json(total_nanos.div_euclid(1_000_000)),
        "microseconds": i128_json(total_nanos.div_euclid(1_000)),
        "nanoseconds": i128_json(total_nanos),
    })
}

/// The calendar breakdown of a UTC datetime.
fn breakdown_value(dt: &DateTime<Utc>) -> Value {
    let month = dt.month();
    let weekday = dt.weekday().num_days_from_monday() as usize;
    json!({
        "year": dt.year(),
        "month": month,
        "month_name": MONTHS[(month - 1) as usize],
        "day": dt.day(),
        "weekday": WEEKDAYS[weekday],
        "day_of_year": dt.ordinal(),
        "iso_week": dt.iso_week().week(),
        "hour": dt.hour(),
        "minute": dt.minute(),
        "second": dt.second(),
        "nanosecond": dt.nanosecond(),
    })
}

fn rfc2822_safe(dt: &DateTime<Utc>) -> Option<String> {
    // chrono's RFC 2822 formatter is intentionally limited to common
    // four-digit years; forced timestamp units can produce otherwise
    // representable chrono dates outside that formatting range.
    (0..=9999).contains(&dt.year()).then(|| dt.to_rfc2822())
}

fn timestamp_to_date(value: &str, unit: Unit) -> Result<Value, String> {
    if !is_numeric_token(value) {
        return Err(format!(
            "'{value}' is not a numeric Unix timestamp; set mode to to-timestamp to convert a date string"
        ));
    }
    let (nanos, resolved) = token_to_nanos(value, unit)?;
    let dt = datetime_from_nanos(nanos)?;
    Ok(json!({
        "direction": "timestamp-to-date",
        "input": value.trim(),
        "detected_unit": resolved.label(),
        "timestamp": timestamps_value(nanos),
        "utc": dt.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
        "iso8601": dt.to_rfc3339(),
        "rfc2822": rfc2822_safe(&dt),
        "date": dt.format("%Y-%m-%d").to_string(),
        "time": dt.format("%H:%M:%S").to_string(),
        "components": breakdown_value(&dt),
    }))
}

fn date_to_timestamp(value: &str) -> Result<Value, String> {
    let parsed = gizza_ai_parse_datetime_core::parse(value)
        .map_err(|e| format!("could not parse date '{value}': {e}"))?;
    if parsed.kind == "time" {
        return Err(format!(
            "'{value}' is a time of day with no date, so it has no Unix timestamp — include a date"
        ));
    }
    let year = parsed.year.ok_or("the parsed date has no year")?;
    let month = parsed.month.unwrap_or(1);
    let day = parsed.day.unwrap_or(1);
    let hour = parsed.hour.unwrap_or(0);
    let minute = parsed.minute.unwrap_or(0);
    let second = parsed.second.unwrap_or(0);

    let date = NaiveDate::from_ymd_opt(year, month, day)
        .ok_or_else(|| format!("invalid calendar date {year}-{month:02}-{day:02}"))?;
    let time = NaiveTime::from_hms_opt(hour, minute, second)
        .ok_or_else(|| format!("invalid time {hour:02}:{minute:02}:{second:02}"))?;
    let naive = date.and_time(time);

    // A wall clock written with an offset is shifted to UTC; an offset-less time
    // is taken as already UTC.
    let offset_secs = parsed.utc_offset_seconds.unwrap_or(0);
    let utc_naive = naive - chrono::Duration::seconds(offset_secs as i64);
    let dt: DateTime<Utc> = Utc.from_utc_datetime(&utc_naive);

    let nanos = dt.timestamp() as i128 * 1_000_000_000 + dt.timestamp_subsec_nanos() as i128;
    Ok(json!({
        "direction": "date-to-timestamp",
        "input": value.trim(),
        "assumed_utc": parsed.utc_offset_seconds.is_none(),
        "utc_offset_seconds": offset_secs,
        "timestamp": timestamps_value(nanos),
        "utc": dt.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
        "iso8601": dt.to_rfc3339(),
        "rfc2822": rfc2822_safe(&dt),
        "components": breakdown_value(&dt),
    }))
}

/// Convert `value` according to `mode` (auto|to-date|to-timestamp) and `unit`
/// (auto|seconds|milliseconds|microseconds|nanoseconds — only used for the
/// timestamp->date direction). Returns pretty-printed JSON.
pub fn run(value: &str, mode: &str, unit: &str) -> Result<String, String> {
    let direction = Direction::parse(mode)?;
    let unit = Unit::parse(unit)?;
    let v = value.trim();
    if v.is_empty() {
        return Err("value is required".into());
    }
    let effective = match direction {
        Direction::Auto => {
            if is_numeric_token(v) {
                Direction::ToDate
            } else {
                Direction::ToTimestamp
            }
        }
        d => d,
    };
    let out = match effective {
        Direction::ToDate => timestamp_to_date(v, unit)?,
        Direction::ToTimestamp => date_to_timestamp(v)?,
        Direction::Auto => unreachable!(),
    };
    serde_json::to_string_pretty(&out).map_err(|e| format!("serialize failed: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn j(value: &str, mode: &str, unit: &str) -> Value {
        serde_json::from_str(&run(value, mode, unit).unwrap()).unwrap()
    }

    #[test]
    fn seconds_to_date_known_vector() {
        // 1700000000 = Tue, 14 Nov 2023 22:13:20 UTC.
        let v = j("1700000000", "auto", "auto");
        assert_eq!(v["direction"], "timestamp-to-date");
        assert_eq!(v["detected_unit"], "seconds");
        assert_eq!(v["timestamp"]["seconds"], 1700000000i64);
        assert_eq!(v["timestamp"]["milliseconds"], 1700000000000i64);
        assert_eq!(v["utc"], "2023-11-14 22:13:20 UTC");
        assert_eq!(v["iso8601"], "2023-11-14T22:13:20+00:00");
        assert_eq!(v["components"]["year"], 2023);
        assert_eq!(v["components"]["month"], 11);
        assert_eq!(v["components"]["month_name"], "November");
        assert_eq!(v["components"]["day"], 14);
        assert_eq!(v["components"]["weekday"], "Tuesday");
        assert_eq!(v["components"]["hour"], 22);
        assert_eq!(v["components"]["minute"], 13);
        assert_eq!(v["components"]["second"], 20);
    }

    #[test]
    fn epoch_zero_is_thursday_1970() {
        let v = j("0", "to-date", "seconds");
        assert_eq!(v["utc"], "1970-01-01 00:00:00 UTC");
        assert_eq!(v["components"]["weekday"], "Thursday");
        assert_eq!(v["components"]["year"], 1970);
        assert_eq!(v["components"]["day_of_year"], 1);
    }

    #[test]
    fn milliseconds_auto_detected() {
        let v = j("1700000000000", "auto", "auto");
        assert_eq!(v["detected_unit"], "milliseconds");
        assert_eq!(v["timestamp"]["seconds"], 1700000000i64);
        assert_eq!(v["components"]["year"], 2023);
    }

    #[test]
    fn microseconds_and_nanoseconds_auto_detected() {
        assert_eq!(j("1700000000000000", "auto", "auto")["detected_unit"], "microseconds");
        assert_eq!(j("1700000000000000000", "auto", "auto")["detected_unit"], "nanoseconds");
    }

    #[test]
    fn unit_override_changes_interpretation() {
        // Forcing seconds on a 13-digit value lands far in the future, not 2023.
        let v = j("1700000000000", "to-date", "seconds");
        assert_eq!(v["detected_unit"], "seconds");
        assert_ne!(v["components"]["year"], 2023);
    }

    #[test]
    fn fractional_seconds_keep_nanos() {
        let v = j("1700000000.5", "to-date", "seconds");
        assert_eq!(v["timestamp"]["nanoseconds"], 1700000000500000000i64);
        assert_eq!(v["components"]["second"], 20);
        assert_eq!(v["components"]["nanosecond"], 500000000);
    }

    #[test]
    fn negative_timestamp_before_epoch() {
        // -1 second = 1969-12-31 23:59:59 UTC.
        let v = j("-1", "to-date", "seconds");
        assert_eq!(v["utc"], "1969-12-31 23:59:59 UTC");
        assert_eq!(v["components"]["year"], 1969);
    }

    #[test]
    fn date_to_timestamp_assumes_utc() {
        let v = j("2023-11-14 22:13:20", "to-timestamp", "auto");
        assert_eq!(v["direction"], "date-to-timestamp");
        assert_eq!(v["assumed_utc"], true);
        assert_eq!(v["utc_offset_seconds"], 0);
        assert_eq!(v["timestamp"]["seconds"], 1700000000i64);
    }

    #[test]
    fn date_to_timestamp_honors_offset() {
        // 00:13:20 +02:00 is the same instant as 22:13:20 UTC the day before.
        let v = j("2023-11-15T00:13:20+02:00", "to-timestamp", "auto");
        assert_eq!(v["assumed_utc"], false);
        assert_eq!(v["utc_offset_seconds"], 7200);
        assert_eq!(v["timestamp"]["seconds"], 1700000000i64);
    }

    #[test]
    fn auto_mode_routes_text_to_date_parsing() {
        let v = j("January 1, 1970", "auto", "auto");
        assert_eq!(v["direction"], "date-to-timestamp");
        assert_eq!(v["timestamp"]["seconds"], 0i64);
    }

    #[test]
    fn date_only_assumes_midnight() {
        let v = j("2023-11-14", "to-timestamp", "auto");
        assert_eq!(v["utc"], "2023-11-14 00:00:00 UTC");
    }

    #[test]
    fn errors_are_reported() {
        assert!(run("", "auto", "auto").is_err());
        assert!(run("not a date at all", "to-timestamp", "auto").is_err());
        assert!(run("12:30", "to-timestamp", "auto").is_err()); // time-only
        assert!(run("1700000000", "bogus-mode", "auto").is_err());
        assert!(run("1700000000", "to-date", "fortnights").is_err());
        assert!(run("hello", "to-date", "auto").is_err()); // non-numeric for to-date
    }
}
