//! gizza-ai/parse-datetime core — parse a single date/time string in many
//! common formats and return its structured components (year, month, day, hour,
//! minute, second, weekday, ISO 8601). Pure-Rust (`chrono`), no clock, no I/O.
//!
//! Accepts (whitespace-trimmed, case-insensitive month/AM-PM):
//!   * ISO 8601 / RFC 3339: `2024-01-05`, `2024-01-05T14:30:00`,
//!     `2024-01-05T14:30:00Z`, `2024-01-05T14:30:00+02:00`, `2024-01-05 14:30`
//!   * RFC 2822 (email): `Mon, 05 Jan 2024 14:30:00 +0000`
//!   * US slash dates: `01/05/2024`, `1/5/24` (month-first; day-first when
//!     the first field is > 12), optionally with a time: `01/05/2024 2:30 PM`
//!   * European dot dates: `05.01.2024` (day-first)
//!   * `2024/01/05` (year-first)
//!   * month-name dates: `January 5, 2024`, `5 Jan 2024`, `Jan 5 2024`,
//!     `Jan 5, 2024 2:30 PM`
//!   * bare clock times: `14:30`, `2:30:15`, `3pm`, `9:05 AM` (no date)
//!
//! Numeric dates are read month-first (US style) unless the first field is
//! > 12 (then day-first). Dotted dates are read day-first (European style).
//! Two-digit years map 00..=69 -> 2000.., 70..=99 -> 1900...

use chrono::{Datelike, NaiveDate, NaiveDateTime, NaiveTime, Timelike};
use serde::Serialize;

/// The structured result of parsing a date/time string.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Parsed {
    /// What was recognized: `"datetime"`, `"date"`, or `"time"`.
    pub kind: String,

    // --- date fields (present for date/datetime) ---
    #[serde(skip_serializing_if = "Option::is_none")]
    pub year: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub month: Option<u32>,
    /// English month name (e.g. `"January"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub month_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub day: Option<u32>,
    /// English weekday name (e.g. `"Friday"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weekday: Option<String>,
    /// 1-based day of the year (1..=366).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub day_of_year: Option<u32>,
    /// ISO 8601 week number (1..=53).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iso_week: Option<u32>,

    // --- time fields (present for time/datetime) ---
    /// Hour in 24-hour form (0..=23).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hour: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minute: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub second: Option<u32>,

    /// UTC offset in seconds, if the input carried a timezone (`Z` -> 0).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub utc_offset_seconds: Option<i32>,

    /// Canonical ISO 8601 rendering of what was parsed (date, time, or
    /// datetime, with offset when present).
    pub iso8601: String,
}

const MONTH_NAMES: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

fn month_num(name: &str) -> Option<u32> {
    let n = name.trim_end_matches('.').to_ascii_lowercase();
    let m = match n.as_str() {
        "jan" | "january" => 1,
        "feb" | "february" => 2,
        "mar" | "march" => 3,
        "apr" | "april" => 4,
        "may" => 5,
        "jun" | "june" => 6,
        "jul" | "july" => 7,
        "aug" | "august" => 8,
        "sep" | "sept" | "september" => 9,
        "oct" | "october" => 10,
        "nov" | "november" => 11,
        "dec" | "december" => 12,
        _ => return None,
    };
    Some(m)
}

fn full_year(y: i32) -> i32 {
    if y >= 100 {
        y
    } else if y < 70 {
        2000 + y
    } else {
        1900 + y
    }
}

/// Build a `Parsed` from a fully-resolved date (no time component).
fn from_date(d: NaiveDate) -> Parsed {
    Parsed {
        kind: "date".into(),
        year: Some(d.year()),
        month: Some(d.month()),
        month_name: Some(MONTH_NAMES[(d.month() - 1) as usize].to_string()),
        day: Some(d.day()),
        weekday: Some(d.weekday().to_string_long()),
        day_of_year: Some(d.ordinal()),
        iso_week: Some(d.iso_week().week()),
        hour: None,
        minute: None,
        second: None,
        utc_offset_seconds: None,
        iso8601: d.format("%Y-%m-%d").to_string(),
    }
}

/// Build a `Parsed` from a bare time (no date component).
fn from_time(t: NaiveTime) -> Parsed {
    Parsed {
        kind: "time".into(),
        year: None,
        month: None,
        month_name: None,
        day: None,
        weekday: None,
        day_of_year: None,
        iso_week: None,
        hour: Some(t.hour()),
        minute: Some(t.minute()),
        second: Some(t.second()),
        utc_offset_seconds: None,
        iso8601: t.format("%H:%M:%S").to_string(),
    }
}

/// Build a `Parsed` from a date+time, with an optional UTC offset (seconds).
fn from_datetime(dt: NaiveDateTime, offset: Option<i32>) -> Parsed {
    let d = dt.date();
    let t = dt.time();
    let iso = match offset {
        Some(0) => format!("{}T{}Z", d.format("%Y-%m-%d"), t.format("%H:%M:%S")),
        Some(off) => {
            let sign = if off < 0 { '-' } else { '+' };
            let a = off.unsigned_abs();
            format!(
                "{}T{}{}{:02}:{:02}",
                d.format("%Y-%m-%d"),
                t.format("%H:%M:%S"),
                sign,
                a / 3600,
                (a % 3600) / 60
            )
        }
        None => format!("{}T{}", d.format("%Y-%m-%d"), t.format("%H:%M:%S")),
    };
    Parsed {
        kind: "datetime".into(),
        year: Some(d.year()),
        month: Some(d.month()),
        month_name: Some(MONTH_NAMES[(d.month() - 1) as usize].to_string()),
        day: Some(d.day()),
        weekday: Some(d.weekday().to_string_long()),
        day_of_year: Some(d.ordinal()),
        iso_week: Some(d.iso_week().week()),
        hour: Some(t.hour()),
        minute: Some(t.minute()),
        second: Some(t.second()),
        utc_offset_seconds: offset,
        iso8601: iso,
    }
}

trait WeekdayLong {
    fn to_string_long(&self) -> String;
}
impl WeekdayLong for chrono::Weekday {
    fn to_string_long(&self) -> String {
        use chrono::Weekday::*;
        match self {
            Mon => "Monday",
            Tue => "Tuesday",
            Wed => "Wednesday",
            Thu => "Thursday",
            Fri => "Friday",
            Sat => "Saturday",
            Sun => "Sunday",
        }
        .to_string()
    }
}

/// Parse a clock time (with optional seconds and AM/PM) into HMS, validating.
fn parse_clock(h: u32, m: u32, s: u32, ampm: Option<&str>) -> Option<NaiveTime> {
    if m > 59 || s > 59 {
        return None;
    }
    let h24 = match ampm.map(|a| a.to_ascii_lowercase().replace('.', "")) {
        Some(a) if a.starts_with('p') => {
            if h == 0 || h > 12 {
                return None;
            }
            if h == 12 {
                12
            } else {
                h + 12
            }
        }
        Some(a) if a.starts_with('a') => {
            if h == 0 || h > 12 {
                return None;
            }
            if h == 12 {
                0
            } else {
                h
            }
        }
        _ => {
            if h > 23 {
                return None;
            }
            h
        }
    };
    NaiveTime::from_hms_opt(h24, m, s)
}

/// True if a token looks like a clock fragment: contains a `:` (e.g. `14:30`)
/// or is a bare hour with an am/pm suffix (e.g. `3pm`, `2:30pm`).
fn looks_like_time_token(t: &str) -> bool {
    let tl = t.to_ascii_lowercase();
    if tl.contains(':') {
        return true;
    }
    let body = tl
        .trim_end_matches("a.m.")
        .trim_end_matches("p.m.")
        .trim_end_matches("am")
        .trim_end_matches("pm");
    body != tl && !body.is_empty() && body.chars().all(|c| c.is_ascii_digit())
}

/// True if a token is a standalone am/pm marker.
fn is_ampm_marker(t: &str) -> bool {
    matches!(
        t.to_ascii_lowercase().as_str(),
        "am" | "pm" | "a.m." | "p.m."
    )
}

/// Split a string into a leading date part and a trailing time part. Uses the
/// ISO `T` separator when present; otherwise scans tokens from the right and
/// peels off a trailing time (a clock token, optionally followed by an am/pm
/// word). Returns `(date_part, Some(time_part))` or `(whole, None)`.
fn split_date_time(s: &str) -> (&str, Option<&str>) {
    // ISO 'T' separator.
    if let Some(i) = s.find(['T', 't']) {
        let before = s[..i].chars().last();
        let after = s[i + 1..].chars().next();
        if matches!(before, Some(c) if c.is_ascii_digit())
            && matches!(after, Some(c) if c.is_ascii_digit())
        {
            return (s[..i].trim(), Some(s[i + 1..].trim()));
        }
    }

    // Token scan. Find the byte index where the trailing time begins.
    let toks: Vec<(usize, &str)> = {
        let mut v = Vec::new();
        let mut idx = 0;
        for part in s.split_whitespace() {
            // locate this token's byte offset at or after idx
            let off = s[idx..].find(part).map(|o| idx + o).unwrap_or(idx);
            v.push((off, part));
            idx = off + part.len();
        }
        v
    };
    if toks.len() < 2 {
        return (s.trim(), None);
    }
    let last = toks.last().unwrap();
    // Case: "... 2:30 PM" — last token is am/pm, prior is a clock.
    if is_ampm_marker(last.1) && toks.len() >= 3 {
        let prev = toks[toks.len() - 2];
        if looks_like_time_token(prev.1) {
            return (s[..prev.0].trim(), Some(s[prev.0..].trim()));
        }
    }
    // Case: "... 14:30" or "... 3pm" — last token is itself a time.
    if looks_like_time_token(last.1) {
        return (s[..last.0].trim(), Some(s[last.0..].trim()));
    }
    (s.trim(), None)
}

/// Parse a time fragment like `14:30`, `2:30:15`, `3pm`, `9:05 AM`.
fn parse_time_fragment(s: &str) -> Option<NaiveTime> {
    let s = s.trim();
    let lower = s.to_ascii_lowercase();
    // Detect a trailing am/pm marker.
    let (body, ampm) = if let Some(stripped) = lower.strip_suffix("a.m.").map(|_| &s[..s.len() - 4])
    {
        (stripped, Some("am"))
    } else if let Some(stripped) = lower.strip_suffix("p.m.").map(|_| &s[..s.len() - 4]) {
        (stripped, Some("pm"))
    } else if lower.ends_with("am") {
        (&s[..s.len() - 2], Some("am"))
    } else if lower.ends_with("pm") {
        (&s[..s.len() - 2], Some("pm"))
    } else {
        (s, None)
    };
    let body = body.trim();
    let parts: Vec<&str> = body.split(':').collect();
    let (h, m, sec) = match parts.as_slice() {
        [h] => (h.trim().parse().ok()?, 0u32, 0u32),
        [h, m] => (h.trim().parse().ok()?, m.trim().parse().ok()?, 0u32),
        [h, m, s] => (
            h.trim().parse().ok()?,
            m.trim().parse().ok()?,
            s.trim().parse().ok()?,
        ),
        _ => return None,
    };
    parse_clock(h, m, sec, ampm)
}

/// Parse a date fragment (no time) into a `NaiveDate`. Handles ISO, slash,
/// dotted, year-first, and month-name forms.
fn parse_date_fragment(s: &str) -> Option<NaiveDate> {
    let s = s.trim();

    // ISO: YYYY-MM-DD
    if let Ok(d) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return Some(d);
    }

    // Month-name forms: "January 5, 2024" / "Jan 5 2024" / "5 Jan 2024".
    if let Some(d) = parse_month_name(s) {
        return Some(d);
    }

    // Separator-delimited numeric (slash or dot).
    let sep = if s.contains('/') {
        '/'
    } else if s.contains('.') {
        '.'
    } else if s.contains('-') {
        '-'
    } else {
        return None;
    };
    let fields: Vec<&str> = s.split(sep).map(|x| x.trim()).collect();
    if fields.len() != 3 {
        return None;
    }
    let a: i64 = fields[0].parse().ok()?;
    let b: u32 = fields[1].parse().ok()?;
    let c: i64 = fields[2].parse().ok()?;

    // Year-first if the first field is 4 digits.
    if fields[0].len() == 4 {
        let y = a as i32;
        let d = c as u32;
        return NaiveDate::from_ymd_opt(y, b, d);
    }
    // Else first two fields are day/month in some order, last is year.
    let year = full_year(c as i32);
    let first = a as u32;
    if sep == '.' {
        // European dotted dates are day-first.
        return NaiveDate::from_ymd_opt(year, b, first);
    }
    // Slash/dash: month-first unless the first field is > 12.
    let (mo, day) = if first > 12 && b <= 12 {
        (b, first)
    } else {
        (first, b)
    };
    NaiveDate::from_ymd_opt(year, mo, day)
}

/// Parse the month-name date forms.
fn parse_month_name(s: &str) -> Option<NaiveDate> {
    // Normalize: drop a trailing comma after the day, ordinal suffixes.
    let cleaned: String = s.replace(',', " ");
    let toks: Vec<&str> = cleaned.split_whitespace().collect();
    if toks.len() != 3 {
        return None;
    }
    let strip_ord = |t: &str| {
        let t = t.trim_end_matches(|c: char| c.is_ascii_alphabetic());
        t.to_string()
    };
    // "Month Day Year"
    if let Some(mo) = month_num(toks[0]) {
        let day: u32 = strip_ord(toks[1]).parse().ok()?;
        let y: i32 = toks[2].parse().ok()?;
        return NaiveDate::from_ymd_opt(full_year(y), mo, day);
    }
    // "Day Month Year"
    if let Some(mo) = month_num(toks[1]) {
        let day: u32 = strip_ord(toks[0]).parse().ok()?;
        let y: i32 = toks[2].parse().ok()?;
        return NaiveDate::from_ymd_opt(full_year(y), mo, day);
    }
    None
}

/// Parse a trailing timezone marker (`Z`, `+02:00`, `-0500`, `+02`) off the end
/// of a time fragment. Returns `(remaining_time_str, offset_seconds)`.
fn split_offset(s: &str) -> (&str, Option<i32>) {
    let s = s.trim();
    if let Some(stripped) = s.strip_suffix(['Z', 'z']) {
        return (stripped.trim(), Some(0));
    }
    // Find a +/- that begins an offset (after the seconds). Scan from the end.
    let bytes = s.as_bytes();
    for (i, &b) in bytes.iter().enumerate() {
        if (b == b'+' || b == b'-') && i > 0 {
            let off = &s[i..];
            if let Some(secs) = parse_offset(off) {
                return (s[..i].trim(), Some(secs));
            }
        }
    }
    (s, None)
}

fn parse_offset(s: &str) -> Option<i32> {
    let sign = match s.as_bytes().first()? {
        b'+' => 1,
        b'-' => -1,
        _ => return None,
    };
    let rest = &s[1..];
    let (hh, mm) = if let Some((h, m)) = rest.split_once(':') {
        (h, m)
    } else if rest.len() == 4 {
        (&rest[..2], &rest[2..])
    } else if rest.len() == 2 {
        (rest, "0")
    } else {
        return None;
    };
    let h: i32 = hh.parse().ok()?;
    let m: i32 = mm.parse().ok()?;
    if h > 23 || m > 59 {
        return None;
    }
    Some(sign * (h * 3600 + m * 60))
}

/// Parse a date/time string into structured components.
pub fn parse(input: &str) -> Result<Parsed, String> {
    let s = input.trim();
    if s.is_empty() {
        return Err("empty input".into());
    }

    // RFC 2822 (email) dates: try chrono first.
    if let Ok(dt) = chrono::DateTime::parse_from_rfc2822(s) {
        return Ok(from_datetime(
            dt.naive_local(),
            Some(dt.offset().local_minus_utc()),
        ));
    }
    // RFC 3339 full datetime with offset.
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
        return Ok(from_datetime(dt.naive_local(), Some(dt.offset().local_minus_utc())));
    }

    let (date_part, time_part) = split_date_time(s);

    // If there's a time part, parse date + time.
    if let Some(tp) = time_part {
        let (time_str, offset) = split_offset(tp);
        let date = parse_date_fragment(date_part).ok_or_else(|| {
            format!("could not parse the date part \"{date_part}\"")
        })?;
        let time = parse_time_fragment(time_str)
            .ok_or_else(|| format!("could not parse the time part \"{time_str}\""))?;
        let dt = NaiveDateTime::new(date, time);
        return Ok(from_datetime(dt, offset));
    }

    // No explicit time separator. Try as a date.
    if let Some(d) = parse_date_fragment(date_part) {
        return Ok(from_date(d));
    }

    // Try as a bare time (e.g. "14:30", "3pm").
    if let Some(t) = parse_time_fragment(s) {
        return Ok(from_time(t));
    }

    Err(format!(
        "could not parse \"{s}\" as a date, time, or datetime"
    ))
}

/// Parse and return as pretty JSON (chat / programmatic surface).
pub fn run(input: &str) -> Result<String, String> {
    let p = parse(input)?;
    serde_json::to_string_pretty(&p).map_err(|e| e.to_string())
}

/// Human-readable rendering (used by the page).
pub fn render(input: &str) -> Result<String, String> {
    let p = parse(input)?;
    let mut out = String::new();
    let row = |out: &mut String, label: &str, val: String| {
        out.push_str(&format!("{label:<16}{val}\n"));
    };
    row(&mut out, "Kind:", p.kind.clone());
    row(&mut out, "ISO 8601:", p.iso8601.clone());
    if let Some(y) = p.year {
        row(&mut out, "Year:", y.to_string());
    }
    if let (Some(m), Some(name)) = (p.month, &p.month_name) {
        row(&mut out, "Month:", format!("{m} ({name})"));
    }
    if let Some(d) = p.day {
        row(&mut out, "Day:", d.to_string());
    }
    if let Some(w) = &p.weekday {
        row(&mut out, "Weekday:", w.clone());
    }
    if let Some(h) = p.hour {
        let m = p.minute.unwrap_or(0);
        let s = p.second.unwrap_or(0);
        row(&mut out, "Time:", format!("{h:02}:{m:02}:{s:02}"));
        row(&mut out, "Hour:", h.to_string());
        row(&mut out, "Minute:", m.to_string());
        row(&mut out, "Second:", s.to_string());
    }
    if let Some(off) = p.utc_offset_seconds {
        let sign = if off < 0 { '-' } else { '+' };
        let a = off.unsigned_abs();
        row(
            &mut out,
            "UTC offset:",
            format!("{sign}{:02}:{:02}", a / 3600, (a % 3600) / 60),
        );
    }
    if let Some(doy) = p.day_of_year {
        row(&mut out, "Day of year:", doy.to_string());
    }
    if let Some(w) = p.iso_week {
        row(&mut out, "ISO week:", w.to_string());
    }
    Ok(out.trim_end().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_date() {
        let p = parse("2024-01-05").unwrap();
        assert_eq!(p.kind, "date");
        assert_eq!(p.year, Some(2024));
        assert_eq!(p.month, Some(1));
        assert_eq!(p.month_name.as_deref(), Some("January"));
        assert_eq!(p.day, Some(5));
        assert_eq!(p.weekday.as_deref(), Some("Friday"));
        assert_eq!(p.iso8601, "2024-01-05");
        assert_eq!(p.hour, None);
    }

    #[test]
    fn iso_datetime() {
        let p = parse("2024-01-05T14:30:00").unwrap();
        assert_eq!(p.kind, "datetime");
        assert_eq!(p.hour, Some(14));
        assert_eq!(p.minute, Some(30));
        assert_eq!(p.second, Some(0));
        assert_eq!(p.iso8601, "2024-01-05T14:30:00");
        assert_eq!(p.utc_offset_seconds, None);
    }

    #[test]
    fn iso_datetime_space_separator() {
        let p = parse("2024-01-05 14:30").unwrap();
        assert_eq!(p.kind, "datetime");
        assert_eq!(p.hour, Some(14));
        assert_eq!(p.minute, Some(30));
        assert_eq!(p.iso8601, "2024-01-05T14:30:00");
    }

    #[test]
    fn rfc3339_with_offset() {
        let p = parse("2024-01-05T14:30:00+02:00").unwrap();
        assert_eq!(p.utc_offset_seconds, Some(7200));
        assert_eq!(p.iso8601, "2024-01-05T14:30:00+02:00");
        assert_eq!(p.hour, Some(14));
    }

    #[test]
    fn rfc3339_zulu() {
        let p = parse("2024-01-05T14:30:00Z").unwrap();
        assert_eq!(p.utc_offset_seconds, Some(0));
        assert_eq!(p.iso8601, "2024-01-05T14:30:00Z");
    }

    #[test]
    fn rfc2822_email() {
        let p = parse("Fri, 05 Jan 2024 14:30:00 +0000").unwrap();
        assert_eq!(p.kind, "datetime");
        assert_eq!(p.year, Some(2024));
        assert_eq!(p.month, Some(1));
        assert_eq!(p.day, Some(5));
        assert_eq!(p.hour, Some(14));
        assert_eq!(p.utc_offset_seconds, Some(0));
    }

    #[test]
    fn us_slash_month_first() {
        let p = parse("01/05/2024").unwrap();
        assert_eq!(p.month, Some(1));
        assert_eq!(p.day, Some(5));
        assert_eq!(p.year, Some(2024));
    }

    #[test]
    fn us_slash_day_first_when_first_gt_12() {
        let p = parse("25/12/2024").unwrap();
        assert_eq!(p.month, Some(12));
        assert_eq!(p.day, Some(25));
    }

    #[test]
    fn two_digit_year() {
        let p = parse("3/4/99").unwrap();
        assert_eq!(p.year, Some(1999));
        assert_eq!(p.month, Some(3));
        assert_eq!(p.day, Some(4));
    }

    #[test]
    fn european_dotted_day_first() {
        let p = parse("05.01.2024").unwrap();
        assert_eq!(p.day, Some(5));
        assert_eq!(p.month, Some(1));
        assert_eq!(p.year, Some(2024));
    }

    #[test]
    fn year_first_slash() {
        let p = parse("2024/01/05").unwrap();
        assert_eq!(p.year, Some(2024));
        assert_eq!(p.month, Some(1));
        assert_eq!(p.day, Some(5));
    }

    #[test]
    fn month_name_variants() {
        assert_eq!(parse("January 5, 2024").unwrap().month, Some(1));
        assert_eq!(parse("5 Jan 2024").unwrap().day, Some(5));
        assert_eq!(parse("Jan 5 2024").unwrap().month, Some(1));
        assert_eq!(parse("Dec. 25 1999").unwrap().year, Some(1999));
    }

    #[test]
    fn slash_date_with_time() {
        let p = parse("01/05/2024 2:30 PM").unwrap();
        assert_eq!(p.kind, "datetime");
        assert_eq!(p.hour, Some(14));
        assert_eq!(p.minute, Some(30));
        assert_eq!(p.month, Some(1));
    }

    #[test]
    fn month_name_with_time() {
        let p = parse("Jan 5, 2024 2:30 PM").unwrap();
        assert_eq!(p.kind, "datetime");
        assert_eq!(p.hour, Some(14));
        assert_eq!(p.day, Some(5));
    }

    #[test]
    fn bare_times() {
        let p = parse("3pm").unwrap();
        assert_eq!(p.kind, "time");
        assert_eq!(p.hour, Some(15));
        assert_eq!(p.iso8601, "15:00:00");

        let p = parse("9:05 AM").unwrap();
        assert_eq!(p.hour, Some(9));
        assert_eq!(p.minute, Some(5));

        let p = parse("14:30:15").unwrap();
        assert_eq!(p.hour, Some(14));
        assert_eq!(p.second, Some(15));

        let p = parse("12am").unwrap();
        assert_eq!(p.hour, Some(0));
    }

    #[test]
    fn weekday_and_day_of_year() {
        let p = parse("2024-12-31").unwrap();
        assert_eq!(p.weekday.as_deref(), Some("Tuesday"));
        assert_eq!(p.day_of_year, Some(366)); // 2024 is a leap year
    }

    #[test]
    fn invalid_dates_rejected() {
        assert!(parse("2024-13-40").is_err());
        assert!(parse("Feb 30, 2024").is_err());
        assert!(parse("25:99").is_err());
        assert!(parse("").is_err());
        assert!(parse("not a date").is_err());
    }

    #[test]
    fn render_and_json() {
        let r = render("2024-01-05T14:30:00Z").unwrap();
        assert!(r.contains("ISO 8601:"));
        assert!(r.contains("2024-01-05T14:30:00Z"));
        assert!(r.contains("Friday"));
        let j = run("2024-01-05").unwrap();
        assert!(j.contains("\"year\": 2024"));
        assert!(j.contains("\"weekday\": \"Friday\""));
    }
}
