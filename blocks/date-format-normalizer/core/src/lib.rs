//! date-format-normalizer core — pure compute, shared by the chat skill block and the web page.
//!
//! Finds every date string inside a block of free text — prose, notes, exported
//! records, scraped tables — and rewrites all of them into ONE chosen format,
//! leaving the surrounding words exactly as they were.
//!
//! Recognized on input (detection is per occurrence, so one paste can mix them):
//!
//! | kind | examples |
//! |---|---|
//! | `iso-8601` | `2024-01-05`, `2024-1-5`, `2024-01-05T14:30:00Z`, `2024-01-05 14:30` |
//! | `numeric` | `01/05/2024`, `5.1.2024`, `5-1-24`, `2024/01/05` |
//! | `month name` | `January 5, 2024`, `Jan. 5 2024`, `5 Jan 2024`, `Friday, 5 January 2024 14:30 +0100` |
//! | `timestamp` | bare `1704465000` / `1704465000123` (opt-in — a 10-digit number is usually not a date) |
//!
//! The hard part is not the rendering, it is deciding what `03/04/2024` means.
//! `input_order = "auto"` reads every numeric date in the text first, uses the
//! ones that can only be one thing (a field above 12) to settle day-first vs
//! month-first for the whole document, and only then rewrites. Strings that
//! look like a date but are not one (`2024-02-30`, `13/13/2024`) are never
//! guessed at — they are left exactly as written.
//!
//! Pure-Rust (`regex` + `chrono` + `chrono-tz`); no wafer/wasm-bindgen deps, no
//! clock, no I/O, so the same input always produces byte-identical output.

use chrono::format::{Item, StrftimeItems};
use chrono::{Datelike, FixedOffset, NaiveDate, NaiveTime, Offset, TimeZone, Timelike};
use chrono_tz::Tz;
use regex::Regex;
use std::sync::OnceLock;

/// Hard cap on the input size, so a huge paste can't exhaust memory. The chat/CLI
/// schema and the page copy advertise this same bound.
pub const MAX_BYTES: usize = 1_000_000;

/// Lowest epoch-seconds value accepted when `detect_timestamps` is on (1973-03-03).
const MIN_EPOCH_SECS: i64 = 100_000_000;
/// Highest epoch-seconds value accepted when `detect_timestamps` is on (2100-01-01).
const MAX_EPOCH_SECS: i64 = 4_102_444_800;

const MONTHS_FULL: [&str; 12] = [
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
const MONTHS_SHORT: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

// ---------------------------------------------------------------------------
// detected values
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Iso,
    Numeric,
    MonthName,
    Timestamp,
}

impl Kind {
    fn label(self) -> &'static str {
        match self {
            Kind::Iso => "iso-8601",
            Kind::Numeric => "numeric",
            Kind::MonthName => "month name",
            Kind::Timestamp => "timestamp",
        }
    }
}

/// One fully-resolved date (or date+time) ready to be rendered.
#[derive(Clone, Copy)]
struct Val {
    y: i32,
    m: u32,
    d: u32,
    time: Option<NaiveTime>,
    has_seconds: bool,
    offset: Option<FixedOffset>,
}

/// One detected occurrence in the source text.
struct Cand {
    start: usize,
    end: usize,
    kind: Kind,
    /// Already-resolved y/m/d (ISO, year-first numeric, month-name, timestamp).
    fixed: Option<(i32, u32, u32)>,
    /// Unresolved `(first_field, second_field, year)` for a slash/dot/dash date.
    pending: Option<(u32, u32, i32)>,
    /// The numeric date could be read either way — the document order decides it.
    ambiguous: bool,
    /// `Some(true)` = the fields force day-first, `Some(false)` = force month-first.
    forced: Option<bool>,
    time: Option<NaiveTime>,
    has_seconds: bool,
    offset: Option<FixedOffset>,
}

// ---------------------------------------------------------------------------
// options
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum OutFmt {
    Iso,
    Ymd,
    Dmy,
    Mdy,
    MonthDayYear,
    DayMonthYear,
    Rfc2822,
    UnixSeconds,
    UnixMillis,
    Custom,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Order {
    Auto,
    DayFirst,
    MonthFirst,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Zone {
    Source,
    Named(Tz),
    Fixed(FixedOffset),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Text,
    List,
    Report,
}

struct Opts<'a> {
    format: OutFmt,
    custom: &'a str,
    sep: &'a str,
    month_short: bool,
    year_two: bool,
    leading_zeros: bool,
    keep_time: bool,
    time_12h: bool,
    zone: Zone,
}

fn parse_format(s: &str) -> Result<OutFmt, String> {
    Ok(match s.trim() {
        "" | "iso" | "iso8601" => OutFmt::Iso,
        "ymd" => OutFmt::Ymd,
        "dmy" => OutFmt::Dmy,
        "mdy" => OutFmt::Mdy,
        "month_day_year" => OutFmt::MonthDayYear,
        "day_month_year" => OutFmt::DayMonthYear,
        "rfc2822" => OutFmt::Rfc2822,
        "unix_seconds" => OutFmt::UnixSeconds,
        "unix_millis" => OutFmt::UnixMillis,
        "custom" => OutFmt::Custom,
        other => {
            return Err(format!(
                "output_format must be one of iso, ymd, dmy, mdy, month_day_year, day_month_year, rfc2822, unix_seconds, unix_millis, custom — got '{other}'"
            ))
        }
    })
}

fn parse_sep(s: &str) -> Result<&'static str, String> {
    Ok(match s.trim() {
        "" | "dash" | "-" => "-",
        "slash" | "/" => "/",
        "dot" | "." => ".",
        "none" => "",
        "space" => " ",
        other => {
            return Err(format!(
                "separator must be one of dash, slash, dot, none, space — got '{other}'"
            ))
        }
    })
}

fn parse_order(s: &str) -> Result<Order, String> {
    Ok(match s.trim() {
        "" | "auto" => Order::Auto,
        "day_first" => Order::DayFirst,
        "month_first" => Order::MonthFirst,
        other => {
            return Err(format!(
                "input_order must be one of auto, day_first, month_first — got '{other}'"
            ))
        }
    })
}

fn parse_mode(s: &str) -> Result<Mode, String> {
    Ok(match s.trim() {
        "" | "text" => Mode::Text,
        "list" => Mode::List,
        "report" => Mode::Report,
        other => {
            return Err(format!(
                "output_mode must be one of text, list, report — got '{other}'"
            ))
        }
    })
}

fn parse_month_style(s: &str) -> Result<bool, String> {
    Ok(match s.trim() {
        "" | "full" => false,
        "short" => true,
        other => return Err(format!("month_style must be full or short — got '{other}'")),
    })
}

fn parse_year_style(s: &str) -> Result<bool, String> {
    Ok(match s.trim() {
        "" | "four" => false,
        "two" => true,
        other => return Err(format!("year_style must be four or two — got '{other}'")),
    })
}

fn parse_time_style(s: &str) -> Result<bool, String> {
    Ok(match s.trim() {
        "" | "24h" => false,
        "12h" => true,
        other => return Err(format!("time_style must be 24h or 12h — got '{other}'")),
    })
}

/// Parse a fixed UTC offset written as `+02:00`, `-0700`, `+05`, `UTC+5:30`, `Z`.
fn parse_fixed_offset(raw: &str) -> Option<FixedOffset> {
    let mut t = raw.trim();
    if t.eq_ignore_ascii_case("z") || t.eq_ignore_ascii_case("utc") || t.eq_ignore_ascii_case("gmt")
    {
        return FixedOffset::east_opt(0);
    }
    for prefix in ["UTC", "GMT", "utc", "gmt"] {
        if let Some(rest) = t.strip_prefix(prefix) {
            t = rest.trim();
            break;
        }
    }
    let (sign, rest) = match t.as_bytes().first()? {
        b'+' => (1i32, &t[1..]),
        b'-' => (-1i32, &t[1..]),
        _ => return None,
    };
    let digits: String = rest.chars().filter(|c| c.is_ascii_digit()).collect();
    let (hh, mm) = match digits.len() {
        1 | 2 => (digits.parse::<i32>().ok()?, 0),
        3 => (digits[..1].parse::<i32>().ok()?, digits[1..].parse::<i32>().ok()?),
        4 => (digits[..2].parse::<i32>().ok()?, digits[2..].parse::<i32>().ok()?),
        _ => return None,
    };
    if hh > 14 || mm > 59 {
        return None;
    }
    FixedOffset::east_opt(sign * (hh * 3600 + mm * 60))
}

fn parse_zone(s: &str) -> Result<Zone, String> {
    let t = s.trim();
    if t.is_empty() || t.eq_ignore_ascii_case("source") || t.eq_ignore_ascii_case("keep") {
        return Ok(Zone::Source);
    }
    if let Ok(tz) = t.parse::<Tz>() {
        return Ok(Zone::Named(tz));
    }
    if let Some(off) = parse_fixed_offset(t) {
        return Ok(Zone::Fixed(off));
    }
    Err(format!(
        "unknown output_timezone '{t}' — use \"source\" to keep the offset each date was written with, \"UTC\", an IANA name like \"Europe/Berlin\", or a fixed offset like \"+02:00\""
    ))
}

// ---------------------------------------------------------------------------
// regexes
// ---------------------------------------------------------------------------

const WEEKDAY: &str = r"(?:(?:monday|tuesday|wednesday|thursday|friday|saturday|sunday|mon|tues|tue|weds|wed|thurs|thur|thu|fri|sat|sun)\.?,?[ \t]+)?";
const MONTH_ALT: &str = r"(january|february|march|april|may|june|july|august|september|october|november|december|jan|feb|mar|apr|jun|jul|aug|sept|sep|oct|nov|dec)";

// NOTE: none of the date regexes may end in `\b` — an ISO datetime's `T`
// (`2024-01-05T14:30`) is a word character, so a trailing word boundary makes
// the whole date fail to match. The end is guarded by `not_followed_by_digit`
// instead, which is what the boundary was actually there for.
fn re_iso() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"\b(\d{4})-(\d{1,2})-(\d{1,2})").unwrap())
}

fn re_year_first() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"\b(\d{4})[/.](\d{1,2})[/.](\d{1,2})").unwrap())
}

fn re_numeric() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"\b(\d{1,2})[-/.](\d{1,2})[-/.](\d{4}|\d{2})").unwrap())
}

fn re_month_first() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(&format!(
            r"(?i)\b{WEEKDAY}{MONTH_ALT}\.?[ \t]+(\d{{1,2}})(?:st|nd|rd|th)?,?[ \t]+(\d{{4}})"
        ))
        .unwrap()
    })
}

fn re_day_first() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(&format!(
            r"(?i)\b{WEEKDAY}(\d{{1,2}})(?:st|nd|rd|th)?[ \t]+(?:of[ \t]+)?{MONTH_ALT}\.?,?[ \t]+(\d{{4}})"
        ))
        .unwrap()
    })
}

/// A date must not run straight into another digit — `2024-01-055` is not a date.
fn not_followed_by_digit(text: &str, end: usize) -> bool {
    !text[end..].starts_with(|c: char| c.is_ascii_digit())
}

fn re_epoch() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"\b(\d{13}|\d{10})\b").unwrap())
}

/// A clock time glued to the end of a date match: ` 14:30`, `T14:30:00.123Z`,
/// ` at 2:30 PM`, ` 10:15:30 +0100`.
fn re_time_tail() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(
            r"(?i)^(?:T|,?[ \t]{1,3})(?:at[ \t]+)?(\d{1,2}):(\d{2})(?::(\d{2}))?(?:\.\d{1,9})?(?:[ \t]*([ap])\.?m\.?)?(?:[ \t]*(Z|UTC|GMT|[+-]\d{2}:?\d{2}|[+-]\d{2})\b)?",
        )
        .unwrap()
    })
}

fn month_num(name: &str) -> Option<u32> {
    let n = name.trim_end_matches('.').to_ascii_lowercase();
    Some(match n.as_str() {
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
    })
}

// ---------------------------------------------------------------------------
// detection
// ---------------------------------------------------------------------------

/// Try to glue a clock time onto the date that ends at `from`. Returns the new
/// end offset plus the parsed time when one is there.
fn attach_time(text: &str, from: usize) -> (usize, Option<NaiveTime>, bool, Option<FixedOffset>) {
    let rest = &text[from..];
    let caps = match re_time_tail().captures(rest) {
        Some(c) => c,
        None => return (from, None, false, None),
    };
    let whole = caps.get(0).unwrap();
    let mut h: u32 = match caps.get(1).unwrap().as_str().parse() {
        Ok(v) => v,
        Err(_) => return (from, None, false, None),
    };
    let mi: u32 = caps.get(2).unwrap().as_str().parse().unwrap_or(60);
    let sec: Option<u32> = caps.get(3).and_then(|m| m.as_str().parse().ok());
    if let Some(ap) = caps.get(4) {
        if !(1..=12).contains(&h) {
            return (from, None, false, None);
        }
        h %= 12;
        if ap.as_str().eq_ignore_ascii_case("p") {
            h += 12;
        }
    }
    let time = match NaiveTime::from_hms_opt(h, mi, sec.unwrap_or(0)) {
        Some(t) => t,
        None => return (from, None, false, None),
    };
    let offset = caps.get(5).and_then(|m| parse_fixed_offset(m.as_str()));
    // " 2:30 pm." must not swallow the sentence's full stop; "2:30 p.m." keeps its own.
    let matched = whole.as_str();
    let lower = matched.to_ascii_lowercase();
    let mut end = from + whole.end();
    if lower.ends_with("m.") && !lower.ends_with(".m.") {
        end -= 1;
    }
    (end, Some(time), sec.is_some(), offset)
}

fn push_named(
    out: &mut Vec<Cand>,
    text: &str,
    re: &Regex,
    day_group: usize,
    month_group: usize,
    year_group: usize,
) {
    for c in re.captures_iter(text) {
        let whole = c.get(0).unwrap();
        if !not_followed_by_digit(text, whole.end()) {
            continue;
        }
        let m = match month_num(c.get(month_group).unwrap().as_str()) {
            Some(v) => v,
            None => continue,
        };
        let d: u32 = match c.get(day_group).unwrap().as_str().parse() {
            Ok(v) => v,
            Err(_) => continue,
        };
        let y: i32 = match c.get(year_group).unwrap().as_str().parse() {
            Ok(v) => v,
            Err(_) => continue,
        };
        if NaiveDate::from_ymd_opt(y, m, d).is_none() {
            continue;
        }
        let (end, time, has_seconds, offset) = attach_time(text, whole.end());
        out.push(Cand {
            start: whole.start(),
            end,
            kind: Kind::MonthName,
            fixed: Some((y, m, d)),
            pending: None,
            ambiguous: false,
            forced: None,
            time,
            has_seconds,
            offset,
        });
    }
}

fn detect(text: &str, pivot: i64, detect_timestamps: bool) -> Vec<Cand> {
    let mut out: Vec<Cand> = Vec::new();

    // ISO and year-first numeric — unambiguous by construction.
    for re in [re_iso(), re_year_first()] {
        let kind = if std::ptr::eq(re, re_iso()) {
            Kind::Iso
        } else {
            Kind::Numeric
        };
        for c in re.captures_iter(text) {
            let whole = c.get(0).unwrap();
            if !not_followed_by_digit(text, whole.end()) {
                continue;
            }
            let y: i32 = c.get(1).unwrap().as_str().parse().unwrap_or(0);
            let m: u32 = c.get(2).unwrap().as_str().parse().unwrap_or(0);
            let d: u32 = c.get(3).unwrap().as_str().parse().unwrap_or(0);
            if NaiveDate::from_ymd_opt(y, m, d).is_none() {
                continue;
            }
            let (end, time, has_seconds, offset) = attach_time(text, whole.end());
            out.push(Cand {
                start: whole.start(),
                end,
                kind,
                fixed: Some((y, m, d)),
                pending: None,
                ambiguous: false,
                forced: None,
                time,
                has_seconds,
                offset,
            });
        }
    }

    // Month-name forms.
    push_named(&mut out, text, re_month_first(), 2, 1, 3);
    push_named(&mut out, text, re_day_first(), 1, 2, 3);

    // Slash / dot / dash numeric dates — day-vs-month order still undecided.
    for c in re_numeric().captures_iter(text) {
        let whole = c.get(0).unwrap();
        if !not_followed_by_digit(text, whole.end()) {
            continue;
        }
        let a: u32 = c.get(1).unwrap().as_str().parse().unwrap_or(0);
        let b: u32 = c.get(2).unwrap().as_str().parse().unwrap_or(0);
        let ytxt = c.get(3).unwrap().as_str();
        let yraw: i32 = ytxt.parse().unwrap_or(-1);
        let y = if ytxt.len() == 2 {
            if i64::from(yraw) <= pivot {
                2000 + yraw
            } else {
                1900 + yraw
            }
        } else {
            yraw
        };
        let day_first_ok = NaiveDate::from_ymd_opt(y, b, a).is_some();
        let month_first_ok = NaiveDate::from_ymd_opt(y, a, b).is_some();
        if !day_first_ok && !month_first_ok {
            continue;
        }
        let forced = match (day_first_ok, month_first_ok) {
            (true, false) => Some(true),
            (false, true) => Some(false),
            _ => None,
        };
        let (end, time, has_seconds, offset) = attach_time(text, whole.end());
        out.push(Cand {
            start: whole.start(),
            end,
            kind: Kind::Numeric,
            fixed: None,
            pending: Some((a, b, y)),
            ambiguous: forced.is_none(),
            forced,
            time,
            has_seconds,
            offset,
        });
    }

    // Bare epoch values — opt-in, since most 10-digit numbers are not dates.
    if detect_timestamps {
        for c in re_epoch().captures_iter(text) {
            let whole = c.get(0).unwrap();
            let raw = whole.as_str();
            let n: i64 = match raw.parse() {
                Ok(v) => v,
                Err(_) => continue,
            };
            let (secs, sub_ms) = if raw.len() == 13 {
                (n.div_euclid(1000), n.rem_euclid(1000))
            } else {
                (n, 0)
            };
            if !(MIN_EPOCH_SECS..=MAX_EPOCH_SECS).contains(&secs) {
                continue;
            }
            let dt = match chrono::DateTime::from_timestamp(secs, 0) {
                Some(v) => v,
                None => continue,
            };
            let _ = sub_ms;
            let naive = dt.naive_utc();
            out.push(Cand {
                start: whole.start(),
                end: whole.end(),
                kind: Kind::Timestamp,
                fixed: Some((naive.year(), naive.month(), naive.day())),
                pending: None,
                ambiguous: false,
                forced: None,
                time: Some(naive.time()),
                has_seconds: true,
                offset: FixedOffset::east_opt(0),
            });
        }
    }

    // Leftmost-longest wins; overlapping runners-up are dropped.
    out.sort_by(|x, y| x.start.cmp(&y.start).then((y.end - y.start).cmp(&(x.end - x.start))));
    let mut kept: Vec<Cand> = Vec::with_capacity(out.len());
    let mut cursor = 0usize;
    for c in out {
        if c.start >= cursor {
            cursor = c.end;
            kept.push(c);
        }
    }
    kept
}

/// Settle day-first vs month-first for the whole text. Returns the chosen order
/// plus a human-readable note for the report header.
fn settle_order(cands: &[Cand], want: Order) -> (bool, String) {
    match want {
        Order::DayFirst => (true, "day-first (set explicitly)".to_string()),
        Order::MonthFirst => (false, "month-first (set explicitly)".to_string()),
        Order::Auto => {
            let day_votes = cands.iter().filter(|c| c.forced == Some(true)).count();
            let month_votes = cands.iter().filter(|c| c.forced == Some(false)).count();
            match (day_votes, month_votes) {
                (0, 0) => (
                    false,
                    "month-first (auto: no date in the text settles it, so the month-first default was used)"
                        .to_string(),
                ),
                (d, 0) => (
                    true,
                    format!("day-first (auto: settled by {d} date(s) with a day above 12)"),
                ),
                (0, m) => (
                    false,
                    format!("month-first (auto: settled by {m} date(s) with a month above 12)"),
                ),
                (d, m) => (
                    false,
                    format!(
                        "month-first (auto: the text disagrees with itself — {d} date(s) read day-first and {m} month-first, so the month-first default was used for the rest)"
                    ),
                ),
            }
        }
    }
}

fn resolve(c: &Cand, day_first: bool) -> Option<Val> {
    let (y, m, d) = match (c.fixed, c.pending) {
        (Some(v), _) => v,
        (None, Some((a, b, y))) => {
            let prefer_day = c.forced.unwrap_or(day_first);
            let (mm, dd) = if prefer_day { (b, a) } else { (a, b) };
            if NaiveDate::from_ymd_opt(y, mm, dd).is_some() {
                (y, mm, dd)
            } else if NaiveDate::from_ymd_opt(y, dd, mm).is_some() {
                (y, dd, mm)
            } else {
                return None;
            }
        }
        _ => return None,
    };
    Some(Val {
        y,
        m,
        d,
        time: c.time,
        has_seconds: c.has_seconds,
        offset: c.offset,
    })
}

/// Move a value that carries an explicit offset into the requested zone. Values
/// with no offset of their own are left exactly where they are.
fn shift_zone(v: Val, zone: Zone) -> Result<Val, String> {
    let (off, target) = match (v.offset, zone) {
        (Some(o), Zone::Named(_)) | (Some(o), Zone::Fixed(_)) => (o, zone),
        _ => return Ok(v),
    };
    let naive = NaiveDate::from_ymd_opt(v.y, v.m, v.d)
        .ok_or_else(|| format!("invalid date {}-{}-{}", v.y, v.m, v.d))?
        .and_time(v.time.unwrap_or_else(|| NaiveTime::from_hms_opt(0, 0, 0).unwrap()));
    let instant = off
        .from_local_datetime(&naive)
        .single()
        .ok_or_else(|| format!("{naive} is not a real instant at offset {off}"))?;
    let new_off = match target {
        Zone::Named(tz) => tz.offset_from_utc_datetime(&instant.naive_utc()).fix(),
        Zone::Fixed(f) => f,
        Zone::Source => off,
    };
    let moved = instant.with_timezone(&new_off);
    Ok(Val {
        y: moved.year(),
        m: moved.month(),
        d: moved.day(),
        time: v.time.map(|_| moved.time()),
        has_seconds: v.has_seconds,
        offset: Some(new_off),
    })
}

// ---------------------------------------------------------------------------
// rendering
// ---------------------------------------------------------------------------

fn num(n: u32, width: usize, pad: bool) -> String {
    if pad {
        format!("{n:0width$}")
    } else {
        n.to_string()
    }
}

fn year_txt(y: i32, two: bool, pad: bool) -> String {
    if two {
        let short = y.rem_euclid(100) as u32;
        num(short, 2, true)
    } else if pad {
        format!("{y:04}")
    } else {
        y.to_string()
    }
}

fn fmt_offset(off: FixedOffset, iso: bool) -> String {
    let total = off.local_minus_utc();
    if iso && total == 0 {
        return "Z".to_string();
    }
    let sign = if total < 0 { '-' } else { '+' };
    let abs = total.abs();
    format!("{sign}{:02}:{:02}", abs / 3600, (abs % 3600) / 60)
}

fn fmt_time(t: NaiveTime, has_seconds: bool, twelve: bool) -> String {
    if twelve {
        let h24 = t.hour();
        let ap = if h24 < 12 { "AM" } else { "PM" };
        let h = if h24 % 12 == 0 { 12 } else { h24 % 12 };
        if has_seconds {
            format!("{h}:{:02}:{:02} {ap}", t.minute(), t.second())
        } else {
            format!("{h}:{:02} {ap}", t.minute())
        }
    } else if has_seconds {
        format!("{:02}:{:02}:{:02}", t.hour(), t.minute(), t.second())
    } else {
        format!("{:02}:{:02}", t.hour(), t.minute())
    }
}

fn to_datetime(v: &Val) -> Result<chrono::DateTime<FixedOffset>, String> {
    let off = v.offset.unwrap_or_else(|| FixedOffset::east_opt(0).unwrap());
    let naive = NaiveDate::from_ymd_opt(v.y, v.m, v.d)
        .ok_or_else(|| format!("invalid date {}-{}-{}", v.y, v.m, v.d))?
        .and_time(v.time.unwrap_or_else(|| NaiveTime::from_hms_opt(0, 0, 0).unwrap()));
    off.from_local_datetime(&naive)
        .single()
        .ok_or_else(|| format!("{naive} is not a real instant"))
}

fn render(v: &Val, o: &Opts) -> Result<String, String> {
    let show_time = o.keep_time && v.time.is_some();
    let time_part = if show_time {
        Some(fmt_time(v.time.unwrap(), v.has_seconds, o.time_12h))
    } else {
        None
    };
    let date_only = match o.format {
        OutFmt::Iso => {
            let mut s = format!("{:04}-{:02}-{:02}", v.y, v.m, v.d);
            if show_time {
                let t = v.time.unwrap();
                s.push('T');
                if v.has_seconds {
                    s.push_str(&format!("{:02}:{:02}:{:02}", t.hour(), t.minute(), t.second()));
                } else {
                    s.push_str(&format!("{:02}:{:02}", t.hour(), t.minute()));
                }
                if let Some(off) = v.offset {
                    s.push_str(&fmt_offset(off, true));
                }
            }
            return Ok(s);
        }
        OutFmt::Ymd => format!(
            "{}{}{}{}{}",
            year_txt(v.y, o.year_two, o.leading_zeros),
            o.sep,
            num(v.m, 2, o.leading_zeros),
            o.sep,
            num(v.d, 2, o.leading_zeros)
        ),
        OutFmt::Dmy => format!(
            "{}{}{}{}{}",
            num(v.d, 2, o.leading_zeros),
            o.sep,
            num(v.m, 2, o.leading_zeros),
            o.sep,
            year_txt(v.y, o.year_two, o.leading_zeros)
        ),
        OutFmt::Mdy => format!(
            "{}{}{}{}{}",
            num(v.m, 2, o.leading_zeros),
            o.sep,
            num(v.d, 2, o.leading_zeros),
            o.sep,
            year_txt(v.y, o.year_two, o.leading_zeros)
        ),
        OutFmt::MonthDayYear => {
            let name = if o.month_short {
                MONTHS_SHORT[(v.m - 1) as usize]
            } else {
                MONTHS_FULL[(v.m - 1) as usize]
            };
            format!(
                "{name} {}, {}",
                v.d,
                year_txt(v.y, o.year_two, o.leading_zeros)
            )
        }
        OutFmt::DayMonthYear => {
            let name = if o.month_short {
                MONTHS_SHORT[(v.m - 1) as usize]
            } else {
                MONTHS_FULL[(v.m - 1) as usize]
            };
            format!(
                "{} {name} {}",
                v.d,
                year_txt(v.y, o.year_two, o.leading_zeros)
            )
        }
        OutFmt::Rfc2822 => return Ok(to_datetime(v)?.to_rfc2822()),
        OutFmt::UnixSeconds => return Ok(to_datetime(v)?.timestamp().to_string()),
        OutFmt::UnixMillis => return Ok(to_datetime(v)?.timestamp_millis().to_string()),
        OutFmt::Custom => {
            let dt = to_datetime(v)?;
            return Ok(dt.format_with_items(StrftimeItems::new(o.custom)).to_string());
        }
    };
    let mut s = date_only;
    if let Some(t) = time_part {
        s.push(' ');
        s.push_str(&t);
        if let Some(off) = v.offset {
            s.push(' ');
            s.push_str(&fmt_offset(off, false));
        }
    }
    Ok(s)
}

// ---------------------------------------------------------------------------
// entry point
// ---------------------------------------------------------------------------

/// Rewrite every date string in `text` into one chosen format.
#[allow(clippy::too_many_arguments)]
pub fn run(
    text: &str,
    output_format: &str,
    custom_format: &str,
    separator: &str,
    month_style: &str,
    year_style: &str,
    leading_zeros: bool,
    input_order: &str,
    two_digit_year_pivot: i64,
    keep_time: bool,
    time_style: &str,
    output_timezone: &str,
    detect_timestamps: bool,
    output_mode: &str,
) -> Result<String, String> {
    if text.trim().is_empty() {
        return Err("text is empty — paste the text whose dates you want normalized".into());
    }
    if text.len() > MAX_BYTES {
        return Err(format!(
            "text is {} bytes, over the {MAX_BYTES} byte limit — split it and run the parts separately",
            text.len()
        ));
    }
    if !(0..=99).contains(&two_digit_year_pivot) {
        return Err(format!(
            "two_digit_year_pivot must be between 0 and 99 — got {two_digit_year_pivot}"
        ));
    }
    let format = parse_format(output_format)?;
    if format == OutFmt::Custom {
        if custom_format.trim().is_empty() {
            return Err(
                "output_format is \"custom\" but custom_format is empty — pass a strftime pattern such as \"%d.%m.%Y\""
                    .into(),
            );
        }
        if StrftimeItems::new(custom_format).any(|i| i == Item::Error) {
            return Err(format!(
                "custom_format '{custom_format}' is not a valid strftime pattern — use fields like %Y, %m, %d, %H, %M, %S, %b, %B, %z"
            ));
        }
    }
    let opts = Opts {
        format,
        custom: custom_format,
        sep: parse_sep(separator)?,
        month_short: parse_month_style(month_style)?,
        year_two: parse_year_style(year_style)?,
        leading_zeros,
        keep_time,
        time_12h: parse_time_style(time_style)?,
        zone: parse_zone(output_timezone)?,
    };
    let order = parse_order(input_order)?;
    let mode = parse_mode(output_mode)?;

    let cands = detect(text, two_digit_year_pivot, detect_timestamps);
    let (day_first, order_note) = settle_order(&cands, order);

    let mut rendered: Vec<(usize, usize, String, Kind, bool)> = Vec::with_capacity(cands.len());
    for c in &cands {
        let Some(v) = resolve(c, day_first) else {
            continue;
        };
        let v = shift_zone(v, opts.zone)?;
        rendered.push((c.start, c.end, render(&v, &opts)?, c.kind, c.ambiguous));
    }

    match mode {
        Mode::Text => {
            let mut out = String::with_capacity(text.len());
            let mut at = 0usize;
            for (start, end, txt, _, _) in &rendered {
                out.push_str(&text[at..*start]);
                out.push_str(txt);
                at = *end;
            }
            out.push_str(&text[at..]);
            Ok(out)
        }
        Mode::List => Ok(rendered
            .iter()
            .map(|(_, _, t, _, _)| t.as_str())
            .collect::<Vec<_>>()
            .join("\n")),
        Mode::Report => {
            let mut out = String::new();
            if rendered.is_empty() {
                out.push_str("# no date strings detected\n");
                return Ok(out);
            }
            let mut counts: Vec<(&'static str, usize)> = Vec::new();
            for (_, _, _, kind, _) in &rendered {
                match counts.iter_mut().find(|(k, _)| *k == kind.label()) {
                    Some(e) => e.1 += 1,
                    None => counts.push((kind.label(), 1)),
                }
            }
            let ambiguous = rendered.iter().filter(|(_, _, _, _, a)| *a).count();
            out.push_str(&format!("# {} date string(s) detected\n", rendered.len()));
            out.push_str(&format!(
                "# detected as: {}\n",
                counts
                    .iter()
                    .map(|(k, n)| format!("{k} {n}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
            out.push_str(&format!("# numeric day/month order: {order_note}\n"));
            if ambiguous > 0 {
                out.push_str(&format!(
                    "# {ambiguous} numeric date(s) could be read either way and used that order\n"
                ));
            }
            for (start, end, txt, kind, amb) in &rendered {
                let before = &text[..*start];
                let line = before.matches('\n').count() + 1;
                let col = before.rsplit('\n').next().unwrap_or("").chars().count() + 1;
                let src = text[*start..*end].replace('\n', " ");
                let tag = if *amb {
                    format!("{}, ambiguous", kind.label())
                } else {
                    kind.label().to_string()
                };
                out.push_str(&format!("line {line}, col {col}\t{src}\t->\t{txt}\t({tag})\n"));
            }
            Ok(out)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn iso(text: &str) -> String {
        run(
            text, "iso", "", "dash", "full", "four", true, "auto", 68, true, "24h", "source",
            false, "text",
        )
        .unwrap()
    }

    #[test]
    fn rewrites_mixed_formats_in_place() {
        let got = iso("Invoice dated 03/04/2024, shipped 15/04/2024, paid 22 April 2024.");
        assert_eq!(
            got,
            "Invoice dated 2024-04-03, shipped 2024-04-15, paid 2024-04-22."
        );
    }

    #[test]
    fn auto_order_settles_day_first_from_an_unambiguous_date() {
        // 15/04 can only be day-first, so 03/04 is read the same way.
        let got = iso("03/04/2024 and 15/04/2024");
        assert_eq!(got, "2024-04-03 and 2024-04-15");
    }

    #[test]
    fn auto_order_settles_month_first_from_an_unambiguous_date() {
        let got = iso("03/04/2024 and 04/15/2024");
        assert_eq!(got, "2024-03-04 and 2024-04-15");
    }

    #[test]
    fn explicit_order_overrides_auto() {
        let got = run(
            "03/04/2024", "iso", "", "dash", "full", "four", true, "day_first", 68, true, "24h",
            "source", false, "text",
        )
        .unwrap();
        assert_eq!(got, "2024-04-03");
    }

    #[test]
    fn month_name_and_weekday_prefix() {
        assert_eq!(iso("Friday, January 5, 2024 was busy"), "2024-01-05 was busy");
        assert_eq!(iso("due 5 Jan 2024"), "due 2024-01-05");
        assert_eq!(iso("Jan. 5th, 2024"), "2024-01-05");
    }

    #[test]
    fn keeps_times_and_offsets() {
        assert_eq!(
            iso("starts 2024-01-05T14:30:00Z sharp"),
            "starts 2024-01-05T14:30:00Z sharp"
        );
        assert_eq!(iso("5 Jan 2024 at 2:30 PM."), "2024-01-05T14:30.");
        assert_eq!(
            iso("Fri, 05 Jan 2024 14:30:00 +0100"),
            "2024-01-05T14:30:00+01:00"
        );
    }

    #[test]
    fn drops_times_when_asked() {
        let got = run(
            "2024-01-05T14:30:00Z", "iso", "", "dash", "full", "four", true, "auto", 68, false,
            "24h", "source", false, "text",
        )
        .unwrap();
        assert_eq!(got, "2024-01-05");
    }

    #[test]
    fn every_output_format_renders() {
        let cases = [
            ("iso", "2024-01-05"),
            ("ymd", "2024-01-05"),
            ("dmy", "05-01-2024"),
            ("mdy", "01-05-2024"),
            ("month_day_year", "January 5, 2024"),
            ("day_month_year", "5 January 2024"),
            ("rfc2822", "Fri, 5 Jan 2024 00:00:00 +0000"),
            ("unix_seconds", "1704412800"),
            ("unix_millis", "1704412800000"),
        ];
        for (fmt, want) in cases {
            let got = run(
                "5 Jan 2024", fmt, "", "dash", "full", "four", true, "auto", 68, true, "24h",
                "source", false, "text",
            )
            .unwrap();
            assert_eq!(got, want, "format {fmt}");
        }
    }

    #[test]
    fn separator_month_year_and_zero_padding_options() {
        let got = run(
            "5 Jan 2024", "dmy", "", "slash", "full", "two", true, "auto", 68, true, "24h",
            "source", false, "text",
        )
        .unwrap();
        assert_eq!(got, "05/01/24");
        let got = run(
            "5 Jan 2024", "dmy", "", "dot", "full", "four", false, "auto", 68, true, "24h",
            "source", false, "text",
        )
        .unwrap();
        assert_eq!(got, "5.1.2024");
        let got = run(
            "5 Jan 2024", "ymd", "", "none", "full", "four", true, "auto", 68, true, "24h",
            "source", false, "text",
        )
        .unwrap();
        assert_eq!(got, "20240105");
        let got = run(
            "5 Jan 2024", "month_day_year", "", "dash", "short", "four", true, "auto", 68, true,
            "24h", "source", false, "text",
        )
        .unwrap();
        assert_eq!(got, "Jan 5, 2024");
    }

    #[test]
    fn custom_strftime_pattern() {
        let got = run(
            "5 Jan 2024", "custom", "%d.%m.%Y", "dash", "full", "four", true, "auto", 68, true,
            "24h", "source", false, "text",
        )
        .unwrap();
        assert_eq!(got, "05.01.2024");
    }

    #[test]
    fn twelve_hour_times_and_two_digit_year_pivot() {
        let got = run(
            "2024-01-05 14:30", "dmy", "", "slash", "full", "four", true, "auto", 68, true, "12h",
            "source", false, "text",
        )
        .unwrap();
        assert_eq!(got, "05/01/2024 2:30 PM");
        // 70 > pivot 68 → 1970; with pivot 80 it becomes 2070.
        let got = run(
            "1/5/70", "iso", "", "dash", "full", "four", true, "month_first", 68, true, "24h",
            "source", false, "text",
        )
        .unwrap();
        assert_eq!(got, "1970-01-05");
        let got = run(
            "1/5/70", "iso", "", "dash", "full", "four", true, "month_first", 80, true, "24h",
            "source", false, "text",
        )
        .unwrap();
        assert_eq!(got, "2070-01-05");
    }

    #[test]
    fn converts_zoned_values_to_another_timezone() {
        let got = run(
            "2024-01-05T14:30:00Z", "iso", "", "dash", "full", "four", true, "auto", 68, true,
            "24h", "Europe/Berlin", false, "text",
        )
        .unwrap();
        assert_eq!(got, "2024-01-05T15:30:00+01:00");
        // A zone-less value is left where it is.
        let got = run(
            "2024-01-05 14:30", "iso", "", "dash", "full", "four", true, "auto", 68, true, "24h",
            "Europe/Berlin", false, "text",
        )
        .unwrap();
        assert_eq!(got, "2024-01-05T14:30");
    }

    #[test]
    fn timestamps_are_opt_in() {
        assert_eq!(iso("order 1704465000 shipped"), "order 1704465000 shipped");
        let got = run(
            "order 1704465000 shipped", "iso", "", "dash", "full", "four", true, "auto", 68, true,
            "24h", "source", true, "text",
        )
        .unwrap();
        assert_eq!(got, "order 2024-01-05T14:30:00Z shipped");
    }

    #[test]
    fn list_and_report_modes() {
        let got = run(
            "a 2024-01-05 b 06/01/2024", "iso", "", "dash", "full", "four", true, "auto", 68,
            true, "24h", "source", false, "list",
        )
        .unwrap();
        assert_eq!(got, "2024-01-05\n2024-06-01");
        let got = run(
            "a 2024-01-05 b 06/01/2024", "iso", "", "dash", "full", "four", true, "auto", 68,
            true, "24h", "source", false, "report",
        )
        .unwrap();
        assert!(got.contains("# 2 date string(s) detected"), "{got}");
        assert!(got.contains("line 1, col 3"), "{got}");
        assert!(got.contains("ambiguous"), "{got}");
    }

    #[test]
    fn non_dates_are_left_alone() {
        assert_eq!(iso("build 2024-13-45 and ratio 3/4/5x"), "build 2024-13-45 and ratio 3/4/5x");
    }

    #[test]
    fn rejects_bad_arguments() {
        assert!(run(
            "", "iso", "", "dash", "full", "four", true, "auto", 68, true, "24h", "source", false,
            "text"
        )
        .unwrap_err()
        .contains("empty"));
        assert!(run(
            "5 Jan 2024", "klingon", "", "dash", "full", "four", true, "auto", 68, true, "24h",
            "source", false, "text"
        )
        .unwrap_err()
        .contains("output_format"));
        assert!(run(
            "5 Jan 2024", "custom", "", "dash", "full", "four", true, "auto", 68, true, "24h",
            "source", false, "text"
        )
        .unwrap_err()
        .contains("custom_format is empty"));
        assert!(run(
            "5 Jan 2024", "iso", "", "dash", "full", "four", true, "auto", 68, true, "24h",
            "Mars/Olympus", false, "text"
        )
        .unwrap_err()
        .contains("unknown output_timezone"));
        assert!(run(
            "5 Jan 2024", "iso", "", "dash", "full", "four", true, "auto", 150, true, "24h",
            "source", false, "text"
        )
        .unwrap_err()
        .contains("two_digit_year_pivot"));
    }

    #[test]
    fn rejects_input_over_the_cap() {
        let big = "x".repeat(MAX_BYTES + 1);
        let err = run(
            &big, "iso", "", "dash", "full", "four", true, "auto", 68, true, "24h", "source",
            false, "text",
        )
        .unwrap_err();
        assert!(err.contains("over the"), "{err}");
        // Exactly at the cap is accepted.
        let ok = "y".repeat(MAX_BYTES);
        assert!(run(
            &ok, "iso", "", "dash", "full", "four", true, "auto", 68, true, "24h", "source",
            false, "text"
        )
        .is_ok());
    }
}
