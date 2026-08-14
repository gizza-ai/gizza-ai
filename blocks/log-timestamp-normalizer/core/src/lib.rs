//! log-timestamp-normalizer core — pure compute, shared by the chat skill block
//! and the web page.
//!
//! Takes a block of log text whose lines carry timestamps in whatever formats
//! the producing services happened to use, detects the timestamp on each line,
//! rewrites every one of them into a single chosen output format and timezone,
//! and annotates the elapsed time between consecutive log events.
//!
//! Detection is per line, so one paste can mix formats. Eight source shapes are
//! recognised, tried most-specific first:
//!
//! | kind | example |
//! |---|---|
//! | `apache` | `10/Oct/2000:13:55:36 -0700` |
//! | `iso-8601` | `2023-12-01T10:15:30.123Z`, `2023-12-01 10:15:30` |
//! | `rfc-2822` | `Fri, 01 Dec 2023 10:15:30 +0000` (also the HTTP date) |
//! | `slash` | `2023/12/01 10:15:30` |
//! | `syslog` | `Dec  1 10:15:30` (no year, no zone) |
//! | `epoch-s` / `epoch-ms` / `epoch-us` / `epoch-ns` | `1701425730`, `1701425730123`, … |
//!
//! Timestamps that carry no zone are interpreted in `assume_timezone`; syslog
//! stamps carry no year either, so the year comes from `assume_year` or, when
//! that is `0`, from the nearest dated line in the same paste.
//!
//! No clock and no I/O: every instant comes out of the pasted text, so the same
//! input always produces byte-identical output.

use chrono::{DateTime, Datelike, Duration, FixedOffset, LocalResult, NaiveDate, NaiveDateTime};
use chrono::{Offset, TimeZone, Utc};
use chrono_tz::Tz;
use regex::Regex;
use std::sync::OnceLock;

/// Hard cap on lines processed in one call, so a huge paste can't blow up
/// memory. The chat/CLI schema and the page copy advertise this same bound.
pub const MAX_LINES: usize = 50_000;

/// Lowest year accepted for `assume_year` (0 means "infer from the paste").
pub const MIN_ASSUME_YEAR: i64 = 1970;
/// Highest year accepted for `assume_year`.
pub const MAX_ASSUME_YEAR: i64 = 2100;

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

/// A resolved target/source timezone: either a named IANA zone (DST-aware) or a
/// fixed UTC offset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Zone {
    Named(Tz),
    Fixed(FixedOffset),
}

/// How a normalized timestamp is written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OutFormat {
    Iso8601,
    Iso8601Ms,
    EpochSeconds,
    EpochMillis,
    Rfc2822,
    DateTime,
}

/// Output ordering of the log events.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SortOrder {
    Input,
    Oldest,
    Newest,
}

/// What each output line is made of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OutputMode {
    /// Rewrite the timestamp where it sits, leaving the rest of the line alone.
    Replace,
    /// Prepend the normalized timestamp, leaving the original line untouched.
    Prefix,
    /// Emit only the normalized timestamp (plus its delta).
    TimestampOnly,
}

/// How an elapsed-time annotation is rendered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DeltaFormat {
    Auto,
    Seconds,
    Milliseconds,
    Hms,
}

/// What happens to a line with no detectable timestamp.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Unmatched {
    Keep,
    Drop,
    Mark,
}

/// Suffix appended to a kept line that carries no timestamp under `mark`.
const NO_TIMESTAMP_MARK: &str = "  (no timestamp)";

/// Annotation on the first event in the output (nothing to measure from).
const START_MARK: &str = "start";

fn parse_offset(spec: &str) -> Option<FixedOffset> {
    let trimmed = spec.trim();
    let upper = trimmed.to_ascii_uppercase();
    let rest = if upper.starts_with("UTC") || upper.starts_with("GMT") {
        &trimmed[3..]
    } else {
        trimmed
    };
    let rest = rest.trim();
    let (sign, digits) = match rest.as_bytes().first()? {
        b'+' => (1i32, rest[1..].trim()),
        b'-' => (-1i32, rest[1..].trim()),
        _ => return None,
    };
    let (hours, minutes) = if let Some((h, m)) = digits.split_once(':') {
        (h, m)
    } else if digits.len() == 4 {
        (&digits[..2], &digits[2..])
    } else if !digits.is_empty() && digits.len() <= 2 {
        (digits, "0")
    } else {
        return None;
    };
    let hours: i32 = hours.trim().parse().ok()?;
    let minutes: i32 = minutes.trim().parse().ok()?;
    if hours > 23 || minutes > 59 {
        return None;
    }
    FixedOffset::east_opt(sign * (hours * 3600 + minutes * 60))
}

fn parse_zone(spec: &str, which: &str) -> Result<Zone, String> {
    let name = spec.trim();
    if name.is_empty() {
        return Ok(Zone::Named(Tz::UTC));
    }
    if matches!(
        name.to_ascii_uppercase().as_str(),
        "UTC" | "GMT" | "Z" | "ZULU" | "UT"
    ) {
        return Ok(Zone::Named(Tz::UTC));
    }
    if let Some(offset) = parse_offset(name) {
        return Ok(Zone::Fixed(offset));
    }
    name.parse::<Tz>().map(Zone::Named).map_err(|_| {
        format!(
            "unknown {which} {name:?} — use \"UTC\", an IANA zone name like \
             \"Europe/Berlin\", \"America/New_York\" or \"Asia/Tokyo\", or a \
             fixed offset like \"+02:00\""
        )
    })
}

fn parse_output_format(spec: &str) -> Result<OutFormat, String> {
    match spec.trim().to_ascii_lowercase().as_str() {
        "" | "iso8601" | "iso" | "iso-8601" | "rfc3339" => Ok(OutFormat::Iso8601),
        "iso8601_ms" | "iso8601-ms" | "iso_ms" => Ok(OutFormat::Iso8601Ms),
        "epoch_seconds" | "epoch" | "epoch-seconds" => Ok(OutFormat::EpochSeconds),
        "epoch_millis" | "epoch-millis" | "epoch_ms" => Ok(OutFormat::EpochMillis),
        "rfc2822" | "rfc-2822" | "http" => Ok(OutFormat::Rfc2822),
        "datetime" | "date-time" | "plain" => Ok(OutFormat::DateTime),
        other => Err(format!(
            "unknown output_format {other:?} — use \"iso8601\", \"iso8601_ms\", \
             \"epoch_seconds\", \"epoch_millis\", \"rfc2822\" or \"datetime\""
        )),
    }
}

fn parse_sort(spec: &str) -> Result<SortOrder, String> {
    match spec.trim().to_ascii_lowercase().as_str() {
        "" | "input" | "none" => Ok(SortOrder::Input),
        "oldest" | "asc" | "ascending" => Ok(SortOrder::Oldest),
        "newest" | "desc" | "descending" => Ok(SortOrder::Newest),
        other => Err(format!(
            "unknown sort {other:?} — use \"input\" (leave the order alone), \
             \"oldest\" or \"newest\""
        )),
    }
}

fn parse_output_mode(spec: &str) -> Result<OutputMode, String> {
    match spec.trim().to_ascii_lowercase().as_str() {
        "" | "replace" => Ok(OutputMode::Replace),
        "prefix" => Ok(OutputMode::Prefix),
        "timestamp" | "timestamp-only" | "timestamp_only" => Ok(OutputMode::TimestampOnly),
        other => Err(format!(
            "unknown output_mode {other:?} — use \"replace\", \"prefix\" or \"timestamp\""
        )),
    }
}

fn parse_delta_format(spec: &str) -> Result<DeltaFormat, String> {
    match spec.trim().to_ascii_lowercase().as_str() {
        "" | "auto" => Ok(DeltaFormat::Auto),
        "seconds" | "s" => Ok(DeltaFormat::Seconds),
        "milliseconds" | "ms" => Ok(DeltaFormat::Milliseconds),
        "hms" | "clock" => Ok(DeltaFormat::Hms),
        other => Err(format!(
            "unknown delta_format {other:?} — use \"auto\", \"seconds\", \
             \"milliseconds\" or \"hms\""
        )),
    }
}

fn parse_unmatched(spec: &str) -> Result<Unmatched, String> {
    match spec.trim().to_ascii_lowercase().as_str() {
        "" | "keep" => Ok(Unmatched::Keep),
        "drop" | "remove" => Ok(Unmatched::Drop),
        "mark" | "flag" => Ok(Unmatched::Mark),
        other => Err(format!(
            "unknown unmatched {other:?} — use \"keep\", \"drop\" or \"mark\""
        )),
    }
}

// ---------------------------------------------------------------------------
// Detection
// ---------------------------------------------------------------------------

/// The source format a line's timestamp was written in. Reported in the summary
/// header so a paste's format mix is visible at a glance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Apache,
    Iso8601,
    Rfc2822,
    Slash,
    Syslog,
    EpochSeconds,
    EpochMillis,
    EpochMicros,
    EpochNanos,
    EpochFractional,
}

/// Every kind, in the order the summary lists ties.
const ALL_KINDS: [Kind; 10] = [
    Kind::Apache,
    Kind::EpochFractional,
    Kind::EpochMicros,
    Kind::EpochMillis,
    Kind::EpochNanos,
    Kind::EpochSeconds,
    Kind::Iso8601,
    Kind::Rfc2822,
    Kind::Slash,
    Kind::Syslog,
];

impl Kind {
    fn label(self) -> &'static str {
        match self {
            Kind::Apache => "apache",
            Kind::Iso8601 => "iso-8601",
            Kind::Rfc2822 => "rfc-2822",
            Kind::Slash => "slash",
            Kind::Syslog => "syslog",
            Kind::EpochSeconds => "epoch-s",
            Kind::EpochMillis => "epoch-ms",
            Kind::EpochMicros => "epoch-us",
            Kind::EpochNanos => "epoch-ns",
            Kind::EpochFractional => "epoch-frac",
        }
    }
}

/// What a detected timestamp resolves to before the paste-wide fix-ups.
#[derive(Clone, Copy, Debug)]
enum When {
    /// The stamp carried its own zone (or is an epoch value): already an instant.
    Absolute(DateTime<Utc>),
    /// Zone-less wall-clock time — interpreted in `assume_timezone`.
    Naive(NaiveDateTime),
    /// Syslog: no year either. Resolved from `assume_year` or the paste.
    NoYear {
        month: u32,
        day: u32,
        hour: u32,
        minute: u32,
        second: u32,
        nano: u32,
    },
}

/// One detected timestamp: where it sits on the line, what it was, and when.
#[derive(Clone, Copy, Debug)]
struct Hit {
    start: usize,
    end: usize,
    kind: Kind,
    when: When,
}

fn re_apache() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?i)\b(?P<d>\d{1,2})/(?P<mon>Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec)/(?P<y>\d{4}):(?P<h>\d{2}):(?P<mi>\d{2}):(?P<s>\d{2})(?:[.,](?P<f>\d{1,9}))?(?:\s*(?P<z>[+-]\d{4}))?",
        )
        .expect("apache timestamp pattern")
    })
}

fn re_iso() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"\b(?P<y>\d{4})-(?P<mo>\d{2})-(?P<d>\d{2})[Tt ](?P<h>\d{2}):(?P<mi>\d{2})(?::(?P<s>\d{2})(?:[.,](?P<f>\d{1,9}))?)?(?:(?P<zu>[Zz])\b|\s?(?P<z>[+-]\d{2}:?\d{2}))?",
        )
        .expect("iso-8601 timestamp pattern")
    })
}

fn re_rfc2822() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?i)\b(?:(?:Mon|Tue|Wed|Thu|Fri|Sat|Sun)\b,?\s+)?(?P<d>\d{1,2})\s+(?P<mon>Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec)\b\s+(?P<y>\d{4})\s+(?P<h>\d{2}):(?P<mi>\d{2})(?::(?P<s>\d{2}))?(?:\s+(?P<z>GMT|UTC|UT|Z|[+-]\d{4})\b)?",
        )
        .expect("rfc-2822 timestamp pattern")
    })
}

fn re_slash() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"\b(?P<y>\d{4})/(?P<mo>\d{2})/(?P<d>\d{2})[Tt ](?P<h>\d{2}):(?P<mi>\d{2})(?::(?P<s>\d{2})(?:[.,](?P<f>\d{1,9}))?)?",
        )
        .expect("slash timestamp pattern")
    })
}

fn re_syslog() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?i)\b(?P<mon>Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec)\b\s+(?P<d>\d{1,2})\s+(?P<h>\d{2}):(?P<mi>\d{2}):(?P<s>\d{2})(?:[.,](?P<f>\d{1,9}))?",
        )
        .expect("syslog timestamp pattern")
    })
}

fn re_epoch() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        // Preference order matters: the fractional form first, then longest to
        // shortest, so a 13-digit run is read as milliseconds rather than as a
        // 10-digit seconds value with three stray digits.
        Regex::new(r"\b(?P<v>\d{10}[.,]\d{1,9}|\d{19}|\d{16}|\d{13}|\d{10})\b")
            .expect("epoch timestamp pattern")
    })
}

fn month_number(abbrev: &str) -> Option<u32> {
    let a = abbrev.to_ascii_lowercase();
    Some(match a.as_str() {
        "jan" => 1,
        "feb" => 2,
        "mar" => 3,
        "apr" => 4,
        "may" => 5,
        "jun" => 6,
        "jul" => 7,
        "aug" => 8,
        "sep" => 9,
        "oct" => 10,
        "nov" => 11,
        "dec" => 12,
        _ => return None,
    })
}

/// Fractional-second digits → whole nanoseconds (`.5` is 500 000 000 ns).
fn nanos_from_fraction(digits: &str) -> u32 {
    let mut padded: String = digits.chars().take(9).collect();
    while padded.len() < 9 {
        padded.push('0');
    }
    padded.parse().unwrap_or(0)
}

fn naive_from_parts(
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    second: u32,
    nano: u32,
) -> Option<NaiveDateTime> {
    NaiveDate::from_ymd_opt(year, month, day)?.and_hms_nano_opt(hour, minute, second, nano)
}

fn cap_num(caps: &regex::Captures, name: &str) -> u32 {
    caps.name(name)
        .and_then(|m| m.as_str().parse().ok())
        .unwrap_or(0)
}

fn cap_frac(caps: &regex::Captures) -> u32 {
    caps.name("f")
        .map(|m| nanos_from_fraction(m.as_str()))
        .unwrap_or(0)
}

/// Turn a textual `+HHMM` / `Z` / `GMT` zone token into a fixed offset.
fn offset_from_token(token: &str) -> Option<FixedOffset> {
    let t = token.trim();
    if matches!(t.to_ascii_uppercase().as_str(), "Z" | "GMT" | "UTC" | "UT") {
        return FixedOffset::east_opt(0);
    }
    parse_offset(t)
}

/// Detect the timestamp on one line. Formats are tried most-specific first, and
/// the first one that matches wins — so a line carrying both an ISO stamp and a
/// bare 10-digit id is read as ISO, not as an epoch value.
fn detect(line: &str) -> Option<Hit> {
    if let Some(c) = re_apache().captures(line) {
        let whole = c.get(0)?;
        let month = month_number(c.name("mon")?.as_str())?;
        let naive = naive_from_parts(
            c.name("y")?.as_str().parse().ok()?,
            month,
            cap_num(&c, "d"),
            cap_num(&c, "h"),
            cap_num(&c, "mi"),
            cap_num(&c, "s"),
            cap_frac(&c),
        )?;
        let when = match c.name("z").and_then(|m| offset_from_token(m.as_str())) {
            Some(off) => When::Absolute(fixed_to_utc(naive, off)),
            None => When::Naive(naive),
        };
        return Some(Hit {
            start: whole.start(),
            end: whole.end(),
            kind: Kind::Apache,
            when,
        });
    }

    if let Some(c) = re_iso().captures(line) {
        let whole = c.get(0)?;
        let naive = naive_from_parts(
            c.name("y")?.as_str().parse().ok()?,
            cap_num(&c, "mo"),
            cap_num(&c, "d"),
            cap_num(&c, "h"),
            cap_num(&c, "mi"),
            cap_num(&c, "s"),
            cap_frac(&c),
        )?;
        let offset = if c.name("zu").is_some() {
            FixedOffset::east_opt(0)
        } else {
            c.name("z").and_then(|m| offset_from_token(m.as_str()))
        };
        let when = match offset {
            Some(off) => When::Absolute(fixed_to_utc(naive, off)),
            None => When::Naive(naive),
        };
        return Some(Hit {
            start: whole.start(),
            end: whole.end(),
            kind: Kind::Iso8601,
            when,
        });
    }

    if let Some(c) = re_rfc2822().captures(line) {
        let whole = c.get(0)?;
        let month = month_number(c.name("mon")?.as_str())?;
        let naive = naive_from_parts(
            c.name("y")?.as_str().parse().ok()?,
            month,
            cap_num(&c, "d"),
            cap_num(&c, "h"),
            cap_num(&c, "mi"),
            cap_num(&c, "s"),
            0,
        )?;
        let when = match c.name("z").and_then(|m| offset_from_token(m.as_str())) {
            Some(off) => When::Absolute(fixed_to_utc(naive, off)),
            None => When::Naive(naive),
        };
        return Some(Hit {
            start: whole.start(),
            end: whole.end(),
            kind: Kind::Rfc2822,
            when,
        });
    }

    if let Some(c) = re_slash().captures(line) {
        let whole = c.get(0)?;
        let naive = naive_from_parts(
            c.name("y")?.as_str().parse().ok()?,
            cap_num(&c, "mo"),
            cap_num(&c, "d"),
            cap_num(&c, "h"),
            cap_num(&c, "mi"),
            cap_num(&c, "s"),
            cap_frac(&c),
        )?;
        return Some(Hit {
            start: whole.start(),
            end: whole.end(),
            kind: Kind::Slash,
            when: When::Naive(naive),
        });
    }

    if let Some(c) = re_syslog().captures(line) {
        let whole = c.get(0)?;
        let month = month_number(c.name("mon")?.as_str())?;
        let day = cap_num(&c, "d");
        if !(1..=31).contains(&day) {
            return None;
        }
        return Some(Hit {
            start: whole.start(),
            end: whole.end(),
            kind: Kind::Syslog,
            when: When::NoYear {
                month,
                day,
                hour: cap_num(&c, "h"),
                minute: cap_num(&c, "mi"),
                second: cap_num(&c, "s"),
                nano: cap_frac(&c),
            },
        });
    }

    if let Some(c) = re_epoch().captures(line) {
        let whole = c.get(0)?;
        let raw = c.name("v")?.as_str();
        let (kind, instant) = epoch_instant(raw)?;
        return Some(Hit {
            start: whole.start(),
            end: whole.end(),
            kind,
            when: When::Absolute(instant),
        });
    }

    None
}

/// Read an epoch literal, inferring its precision from the digit count.
fn epoch_instant(raw: &str) -> Option<(Kind, DateTime<Utc>)> {
    if let Some((whole, frac)) = raw.split_once(['.', ',']) {
        let seconds: i64 = whole.parse().ok()?;
        let nanos = nanos_from_fraction(frac);
        return Utc
            .timestamp_opt(seconds, nanos)
            .single()
            .map(|t| (Kind::EpochFractional, t));
    }
    let value: i64 = raw.parse().ok()?;
    let (kind, seconds, nanos) = match raw.len() {
        19 => (
            Kind::EpochNanos,
            value.div_euclid(1_000_000_000),
            value.rem_euclid(1_000_000_000) as u32,
        ),
        16 => (
            Kind::EpochMicros,
            value.div_euclid(1_000_000),
            value.rem_euclid(1_000_000) as u32 * 1_000,
        ),
        13 => (
            Kind::EpochMillis,
            value.div_euclid(1_000),
            value.rem_euclid(1_000) as u32 * 1_000_000,
        ),
        10 => (Kind::EpochSeconds, value, 0),
        _ => return None,
    };
    Utc.timestamp_opt(seconds, nanos)
        .single()
        .map(|t| (kind, t))
}

// ---------------------------------------------------------------------------
// Zone maths
// ---------------------------------------------------------------------------

fn fixed_to_utc(naive: NaiveDateTime, offset: FixedOffset) -> DateTime<Utc> {
    match offset.from_local_datetime(&naive) {
        LocalResult::Single(dt) => dt.with_timezone(&Utc),
        LocalResult::Ambiguous(dt, _) => dt.with_timezone(&Utc),
        LocalResult::None => Utc.from_utc_datetime(&naive),
    }
}

/// Interpret a zone-less wall-clock time in `zone`.
///
/// An ambiguous time (the repeated hour when clocks go back) takes the earlier,
/// still-in-DST instant; a non-existent time (the skipped hour when clocks go
/// forward) moves forward to the first instant that does exist.
fn local_to_utc(naive: NaiveDateTime, zone: Zone) -> DateTime<Utc> {
    match zone {
        Zone::Fixed(offset) => fixed_to_utc(naive, offset),
        Zone::Named(tz) => match tz.from_local_datetime(&naive) {
            LocalResult::Single(dt) => dt.with_timezone(&Utc),
            LocalResult::Ambiguous(earlier, _) => earlier.with_timezone(&Utc),
            LocalResult::None => {
                let mut probe = naive;
                for _ in 0..8 {
                    probe += Duration::minutes(30);
                    if let Some(dt) = tz.from_local_datetime(&probe).earliest() {
                        return dt.with_timezone(&Utc);
                    }
                }
                Utc.from_utc_datetime(&naive)
            }
        },
    }
}

/// Move an instant into `zone` as a fixed-offset datetime, ready to format.
fn to_fixed(instant: DateTime<Utc>, zone: Zone) -> DateTime<FixedOffset> {
    match zone {
        Zone::Fixed(offset) => instant.with_timezone(&offset),
        Zone::Named(tz) => {
            let offset = tz.offset_from_utc_datetime(&instant.naive_utc()).fix();
            instant.with_timezone(&offset)
        }
    }
}

fn render_stamp(instant: DateTime<Utc>, format: OutFormat, zone: Zone) -> String {
    match format {
        OutFormat::EpochSeconds => instant.timestamp().to_string(),
        OutFormat::EpochMillis => instant.timestamp_millis().to_string(),
        OutFormat::Rfc2822 => to_fixed(instant, zone).to_rfc2822(),
        OutFormat::Iso8601 => to_fixed(instant, zone)
            .format("%Y-%m-%dT%H:%M:%S%:z")
            .to_string(),
        OutFormat::Iso8601Ms => to_fixed(instant, zone)
            .format("%Y-%m-%dT%H:%M:%S%.3f%:z")
            .to_string(),
        OutFormat::DateTime => to_fixed(instant, zone)
            .format("%Y-%m-%d %H:%M:%S")
            .to_string(),
    }
}

// ---------------------------------------------------------------------------
// Delta rendering
// ---------------------------------------------------------------------------

/// Whole + fractional seconds with trailing zeros trimmed (`1.2`, not `1.200`).
fn seconds_text(millis: i64) -> String {
    let whole = millis / 1000;
    let frac = millis % 1000;
    if frac == 0 {
        return whole.to_string();
    }
    let mut digits = format!("{frac:03}");
    while digits.ends_with('0') {
        digits.pop();
    }
    format!("{whole}.{digits}")
}

/// An elapsed span, unsigned.
fn duration_text(millis: i64, format: DeltaFormat) -> String {
    let abs = millis.abs();
    match format {
        DeltaFormat::Milliseconds => format!("{abs}ms"),
        DeltaFormat::Seconds => format!("{}s", seconds_text(abs)),
        DeltaFormat::Hms => {
            let secs = abs / 1000;
            format!("{}:{:02}:{:02}", secs / 3600, (secs % 3600) / 60, secs % 60)
        }
        DeltaFormat::Auto => {
            if abs < 1_000 {
                format!("{abs}ms")
            } else if abs < 60_000 {
                format!("{}s", seconds_text(abs))
            } else {
                let secs = abs / 1000;
                if abs < 3_600_000 {
                    format!("{}m{:02}s", secs / 60, secs % 60)
                } else if abs < 86_400_000 {
                    format!(
                        "{}h{:02}m{:02}s",
                        secs / 3600,
                        (secs % 3600) / 60,
                        secs % 60
                    )
                } else {
                    format!(
                        "{}d{:02}h{:02}m",
                        secs / 86_400,
                        (secs % 86_400) / 3600,
                        (secs % 3600) / 60
                    )
                }
            }
        }
    }
}

/// A signed elapsed span, as it appears in the annotation.
fn delta_text(millis: i64, format: DeltaFormat) -> String {
    let sign = if millis < 0 { '-' } else { '+' };
    format!("{sign}{}", duration_text(millis, format))
}

// ---------------------------------------------------------------------------
// Records
// ---------------------------------------------------------------------------

/// One log event: the line whose timestamp was detected, plus the lines that
/// followed it without one (stack traces, wrapped payloads, banners). Keeping
/// them attached means sorting can't tear a traceback away from its event.
struct Record {
    /// 1-based input line number of the head line (or of the first trailing
    /// line, for the preamble record that has no head).
    line_no: usize,
    head: Option<(String, Hit, DateTime<Utc>)>,
    trailing: Vec<String>,
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Normalize every timestamp in `input` and annotate the gaps between events.
///
/// See the module docs for the recognised source formats. Returns the rewritten
/// log text, or a user-facing message describing what was wrong with the
/// arguments.
#[allow(clippy::too_many_arguments)]
pub fn run(
    input: &str,
    output_format: &str,
    output_timezone: &str,
    assume_timezone: &str,
    assume_year: i64,
    sort: &str,
    output_mode: &str,
    delta: bool,
    delta_format: &str,
    gap_threshold_seconds: f64,
    unmatched: &str,
    summary: bool,
) -> Result<String, String> {
    let out_format = parse_output_format(output_format)?;
    let out_zone = parse_zone(output_timezone, "output_timezone")?;
    let in_zone = parse_zone(assume_timezone, "assume_timezone")?;
    let order = parse_sort(sort)?;
    let mode = parse_output_mode(output_mode)?;
    let dfmt = parse_delta_format(delta_format)?;
    let unmatched_policy = parse_unmatched(unmatched)?;

    if assume_year != 0 && !(MIN_ASSUME_YEAR..=MAX_ASSUME_YEAR).contains(&assume_year) {
        return Err(format!(
            "assume_year must be 0 (infer the year from the paste) or between \
             {MIN_ASSUME_YEAR} and {MAX_ASSUME_YEAR}, got {assume_year}"
        ));
    }
    if !gap_threshold_seconds.is_finite() || gap_threshold_seconds < 0.0 {
        return Err(format!(
            "gap_threshold_seconds must be 0 (no gap marking) or a positive \
             number of seconds, got {gap_threshold_seconds}"
        ));
    }

    let body = input.strip_suffix('\n').unwrap_or(input);
    if body.trim().is_empty() {
        return Err("no log text — paste at least one log line".into());
    }
    let lines: Vec<&str> = body.split('\n').map(|l| l.trim_end_matches('\r')).collect();
    if lines.len() > MAX_LINES {
        return Err(format!(
            "{} lines is too many — this tool normalizes at most {MAX_LINES} lines per run",
            lines.len()
        ));
    }

    // Pass 1: detect, and resolve everything that doesn't need the whole paste.
    let hits: Vec<Option<Hit>> = lines.iter().map(|l| detect(l)).collect();
    let mut instants: Vec<Option<DateTime<Utc>>> = hits
        .iter()
        .map(|hit| match hit.map(|h| h.when) {
            Some(When::Absolute(t)) => Some(t),
            Some(When::Naive(n)) => Some(local_to_utc(n, in_zone)),
            Some(When::NoYear { .. }) | None => None,
        })
        .collect();

    // Pass 2: give the year-less (syslog) stamps a year. An explicit
    // `assume_year` wins; otherwise the nearest dated line in the paste does,
    // picking whichever of its year ±1 lands closest — so a December-to-January
    // paste doesn't jump twelve months at the rollover.
    let dated: Vec<(usize, DateTime<Utc>)> = instants
        .iter()
        .enumerate()
        .filter_map(|(i, t)| t.map(|t| (i, t)))
        .collect();
    for (i, hit) in hits.iter().enumerate() {
        let Some(Hit {
            when:
                When::NoYear {
                    month,
                    day,
                    hour,
                    minute,
                    second,
                    nano,
                },
            ..
        }) = hit
        else {
            continue;
        };
        let build = |year: i32| {
            naive_from_parts(year, *month, *day, *hour, *minute, *second, *nano)
                .map(|n| local_to_utc(n, in_zone))
        };
        instants[i] = if assume_year != 0 {
            build(assume_year as i32)
        } else {
            let reference = dated
                .iter()
                .min_by_key(|(j, _)| (j.abs_diff(i), *j))
                .map(|(_, t)| *t);
            reference.and_then(|reference| {
                let base = to_fixed(reference, in_zone).year();
                [base - 1, base, base + 1]
                    .into_iter()
                    .filter_map(build)
                    .min_by_key(|t| (*t - reference).num_seconds().abs())
            })
        };
    }

    // Group each detected event with the un-timestamped lines that follow it.
    let mut records: Vec<Record> = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        match (hits[i], instants[i]) {
            (Some(hit), Some(instant)) => records.push(Record {
                line_no: i + 1,
                head: Some((line.to_string(), hit, instant)),
                trailing: Vec::new(),
            }),
            _ => match records.last_mut() {
                Some(last) => last.trailing.push(line.to_string()),
                None => records.push(Record {
                    line_no: i + 1,
                    head: None,
                    trailing: vec![line.to_string()],
                }),
            },
        }
    }

    // Sort the events. The leading un-timestamped preamble (a banner, a header)
    // has no instant to sort on, so it stays where it is: at the top.
    let preamble = usize::from(records.first().is_some_and(|r| r.head.is_none()));
    match order {
        SortOrder::Input => {}
        SortOrder::Oldest => {
            records[preamble..].sort_by_key(|r| (r.head.as_ref().map(|(_, _, t)| *t), r.line_no))
        }
        SortOrder::Newest => records[preamble..].sort_by_key(|r| {
            (
                std::cmp::Reverse(r.head.as_ref().map(|(_, _, t)| *t)),
                r.line_no,
            )
        }),
    }

    // Render.
    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    let mut previous: Option<DateTime<Utc>> = None;
    let mut largest_gap: Option<(i64, usize)> = None;
    let mut gap_count = 0usize;
    let mut matched = 0usize;
    let mut unmatched_lines = 0usize;
    let mut counts = [0usize; ALL_KINDS.len()];
    let mut earliest: Option<DateTime<Utc>> = None;
    let mut latest: Option<DateTime<Utc>> = None;

    for record in &records {
        if let Some((line, hit, instant)) = &record.head {
            matched += 1;
            if let Some(slot) = ALL_KINDS.iter().position(|k| *k == hit.kind) {
                counts[slot] += 1;
            }
            earliest = Some(earliest.map_or(*instant, |e| e.min(*instant)));
            latest = Some(latest.map_or(*instant, |l| l.max(*instant)));

            let stamp = render_stamp(*instant, out_format, out_zone);
            let mut rendered = match mode {
                OutputMode::Replace => {
                    format!("{}{}{}", &line[..hit.start], stamp, &line[hit.end..])
                }
                OutputMode::Prefix => format!("{stamp}  {line}"),
                OutputMode::TimestampOnly => stamp,
            };

            let elapsed = previous.map(|p| (*instant - p).num_milliseconds());
            if let Some(millis) = elapsed {
                if millis > largest_gap.map_or(-1, |(m, _)| m) {
                    largest_gap = Some((millis, record.line_no));
                }
                if gap_threshold_seconds > 0.0
                    && (millis.abs() as f64) / 1000.0 >= gap_threshold_seconds
                {
                    gap_count += 1;
                }
            }
            if delta {
                let annotation = match elapsed {
                    None => START_MARK.to_string(),
                    Some(millis) => {
                        let flagged = gap_threshold_seconds > 0.0
                            && (millis.abs() as f64) / 1000.0 >= gap_threshold_seconds;
                        let text = delta_text(millis, dfmt);
                        if flagged {
                            format!("{text} GAP")
                        } else {
                            text
                        }
                    }
                };
                rendered.push_str("  (");
                rendered.push_str(&annotation);
                rendered.push(')');
            }
            out.push(rendered);
            previous = Some(*instant);
        }

        for line in &record.trailing {
            unmatched_lines += 1;
            match unmatched_policy {
                Unmatched::Keep => out.push(line.clone()),
                Unmatched::Mark => out.push(format!("{line}{NO_TIMESTAMP_MARK}")),
                Unmatched::Drop => {}
            }
        }
    }

    if summary {
        let mut header = Vec::new();
        header.push(format!(
            "# {} lines · {matched} timestamps · {unmatched_lines} without one",
            lines.len()
        ));
        let mut mix: Vec<(Kind, usize)> = ALL_KINDS
            .iter()
            .zip(counts)
            .filter(|(_, n)| *n > 0)
            .map(|(k, n)| (*k, n))
            .collect();
        mix.sort_by_key(|(kind, n)| (std::cmp::Reverse(*n), kind.label()));
        if !mix.is_empty() {
            let listed: Vec<String> = mix
                .iter()
                .map(|(kind, n)| format!("{} {n}", kind.label()))
                .collect();
            header.push(format!("# formats: {}", listed.join(", ")));
        }
        if let (Some(first), Some(last)) = (earliest, latest) {
            header.push(format!(
                "# span: {} → {} ({})",
                render_stamp(first, out_format, out_zone),
                render_stamp(last, out_format, out_zone),
                duration_text((last - first).num_milliseconds(), dfmt)
            ));
        }
        if let Some((millis, line_no)) = largest_gap {
            header.push(format!(
                "# largest gap: {} at input line {line_no}",
                duration_text(millis, dfmt)
            ));
        }
        if gap_threshold_seconds > 0.0 {
            header.push(format!(
                "# gaps at or above {}s: {gap_count}",
                trim_float(gap_threshold_seconds)
            ));
        }
        header.push(format!(
            "# output: {} in {}",
            format_label(out_format),
            zone_label(out_zone)
        ));
        header.extend(out);
        out = header;
    }

    Ok(out.join("\n"))
}

fn format_label(format: OutFormat) -> &'static str {
    match format {
        OutFormat::Iso8601 => "iso8601",
        OutFormat::Iso8601Ms => "iso8601_ms",
        OutFormat::EpochSeconds => "epoch_seconds",
        OutFormat::EpochMillis => "epoch_millis",
        OutFormat::Rfc2822 => "rfc2822",
        OutFormat::DateTime => "datetime",
    }
}

fn zone_label(zone: Zone) -> String {
    match zone {
        Zone::Named(tz) => tz.name().to_string(),
        Zone::Fixed(offset) => offset.to_string(),
    }
}

/// `60` rather than `60.0`, so the summary reads like the value that was typed.
fn trim_float(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{}", value as i64)
    } else {
        format!("{value}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIXED: &str = "2023-12-01T10:15:30Z INFO  boot: starting service\n\
                         1701425735123 WARN  cache: miss for key user:42\n\
                         Dec  1 10:16:20 web01 nginx: GET /health 200\n\
                         01/Dec/2023:10:17:05 +0000 \"GET /api/orders HTTP/1.1\" 200\n\
                         1701425830 INFO  boot: ready";

    /// The tool's defaults, matching the descriptor and the page.
    fn normalize(input: &str) -> Result<String, String> {
        run(
            input, "iso8601", "UTC", "UTC", 0, "input", "replace", true, "auto", 0.0, "keep", false,
        )
    }

    #[test]
    fn normalizes_a_mixed_format_paste_exactly() {
        assert_eq!(
            normalize(MIXED).unwrap(),
            "2023-12-01T10:15:30+00:00 INFO  boot: starting service  (start)\n\
             2023-12-01T10:15:35+00:00 WARN  cache: miss for key user:42  (+5.123s)\n\
             2023-12-01T10:16:20+00:00 web01 nginx: GET /health 200  (+44.877s)\n\
             2023-12-01T10:17:05+00:00 \"GET /api/orders HTTP/1.1\" 200  (+45s)\n\
             2023-12-01T10:17:10+00:00 INFO  boot: ready  (+5s)"
        );
    }

    // -- source formats ----------------------------------------------------

    fn only_stamp(line: &str) -> String {
        run(
            line,
            "iso8601_ms",
            "UTC",
            "UTC",
            2023,
            "input",
            "timestamp",
            false,
            "auto",
            0.0,
            "keep",
            false,
        )
        .unwrap()
    }

    #[test]
    fn detects_iso_8601_with_and_without_zone_or_separator() {
        assert_eq!(
            only_stamp("2023-12-01T10:15:30Z x"),
            "2023-12-01T10:15:30.000+00:00"
        );
        assert_eq!(
            only_stamp("2023-12-01 10:15:30 x"),
            "2023-12-01T10:15:30.000+00:00"
        );
        assert_eq!(
            only_stamp("2023-12-01T10:15:30.123456Z"),
            "2023-12-01T10:15:30.123+00:00"
        );
        assert_eq!(
            only_stamp("2023-12-01T11:15:30,500+01:00"),
            "2023-12-01T10:15:30.500+00:00"
        );
        assert_eq!(
            only_stamp("2023-12-01T10:15 no seconds"),
            "2023-12-01T10:15:00.000+00:00"
        );
    }

    #[test]
    fn detects_rfc2822_and_http_dates() {
        assert_eq!(
            only_stamp("Fri, 01 Dec 2023 10:15:30 +0000 GET /"),
            "2023-12-01T10:15:30.000+00:00"
        );
        assert_eq!(
            only_stamp("Sun, 06 Nov 1994 08:49:37 GMT"),
            "1994-11-06T08:49:37.000+00:00"
        );
        assert_eq!(
            only_stamp("01 Dec 2023 12:15:30 +0200"),
            "2023-12-01T10:15:30.000+00:00"
        );
    }

    #[test]
    fn detects_apache_bracketed_dates() {
        assert_eq!(
            only_stamp("1.2.3.4 - - [10/Oct/2000:13:55:36 -0700] \"GET / HTTP/1.0\" 200"),
            "2000-10-10T20:55:36.000+00:00"
        );
        // No offset in the bracket → assume_timezone (UTC here).
        assert_eq!(
            only_stamp("[01/Dec/2023:10:17:05] ok"),
            "2023-12-01T10:17:05.000+00:00"
        );
    }

    #[test]
    fn detects_epoch_at_every_precision() {
        assert_eq!(
            only_stamp("ts=1701425730 up"),
            "2023-12-01T10:15:30.000+00:00"
        );
        assert_eq!(
            only_stamp("ts=1701425730123 up"),
            "2023-12-01T10:15:30.123+00:00"
        );
        assert_eq!(
            only_stamp("ts=1701425730123456 up"),
            "2023-12-01T10:15:30.123+00:00"
        );
        assert_eq!(
            only_stamp("ts=1701425730123456789 up"),
            "2023-12-01T10:15:30.123+00:00"
        );
        assert_eq!(
            only_stamp("ts=1701425730.25 up"),
            "2023-12-01T10:15:30.250+00:00"
        );
    }

    #[test]
    fn detects_syslog_and_slash_dates() {
        assert_eq!(
            only_stamp("Dec  1 10:15:30 web01 sshd[1]: ok"),
            "2023-12-01T10:15:30.000+00:00"
        );
        assert_eq!(
            only_stamp("2023/12/01 10:15:30 request done"),
            "2023-12-01T10:15:30.000+00:00"
        );
    }

    #[test]
    fn a_longer_digit_run_is_not_read_as_an_epoch_value() {
        // 11 digits is neither seconds nor milliseconds: no timestamp at all.
        assert_eq!(
            normalize("id=12345678901 nothing here").unwrap(),
            "id=12345678901 nothing here"
        );
        // A word starting with a month abbreviation is not a syslog stamp.
        assert_eq!(
            normalize("Marker 1 10:15:30 elapsed").unwrap(),
            "Marker 1 10:15:30 elapsed"
        );
    }

    #[test]
    fn a_more_specific_format_wins_over_a_bare_number_on_the_same_line() {
        assert_eq!(
            only_stamp("req=1701425730123 done at 2023-12-01T10:20:00Z"),
            "2023-12-01T10:20:00.000+00:00"
        );
    }

    // -- options -----------------------------------------------------------

    #[test]
    fn output_format_and_timezone_control_the_written_stamp() {
        let line = "2023-12-01T10:15:30Z boot";
        let with = |fmt: &str, zone: &str| {
            run(
                line,
                fmt,
                zone,
                "UTC",
                0,
                "input",
                "timestamp",
                false,
                "auto",
                0.0,
                "keep",
                false,
            )
            .unwrap()
        };
        assert_eq!(with("iso8601", "UTC"), "2023-12-01T10:15:30+00:00");
        assert_eq!(
            with("iso8601", "Europe/Berlin"),
            "2023-12-01T11:15:30+01:00"
        );
        assert_eq!(with("datetime", "Asia/Tokyo"), "2023-12-01 19:15:30");
        assert_eq!(with("rfc2822", "UTC"), "Fri, 1 Dec 2023 10:15:30 +0000");
        assert_eq!(with("epoch_seconds", "UTC"), "1701425730");
        assert_eq!(with("epoch_millis", "Asia/Tokyo"), "1701425730000");
        assert_eq!(with("iso8601", "+05:30"), "2023-12-01T15:45:30+05:30");
    }

    #[test]
    fn assume_timezone_anchors_zone_less_stamps_with_dst() {
        // 2023-07-01 is summer time in Berlin (UTC+2), 2023-12-01 is not (UTC+1).
        let summer = run(
            "2023-07-01 12:00:00 job",
            "iso8601",
            "UTC",
            "Europe/Berlin",
            0,
            "input",
            "timestamp",
            false,
            "auto",
            0.0,
            "keep",
            false,
        )
        .unwrap();
        let winter = run(
            "2023-12-01 12:00:00 job",
            "iso8601",
            "UTC",
            "Europe/Berlin",
            0,
            "input",
            "timestamp",
            false,
            "auto",
            0.0,
            "keep",
            false,
        )
        .unwrap();
        assert_eq!(summer, "2023-07-01T10:00:00+00:00");
        assert_eq!(winter, "2023-12-01T11:00:00+00:00");
    }

    #[test]
    fn assume_year_is_explicit_or_inferred_from_the_paste() {
        let explicit = run(
            "Dec  1 10:15:30 web01 sshd[1]: ok",
            "iso8601",
            "UTC",
            "UTC",
            2019,
            "input",
            "timestamp",
            false,
            "auto",
            0.0,
            "keep",
            false,
        )
        .unwrap();
        assert_eq!(explicit, "2019-12-01T10:15:30+00:00");

        // Inferred from the neighbouring dated line, across the new year.
        let inferred = run(
            "Dec 31 23:59:00 web01 cron: nightly\n2024-01-01T00:01:00Z INFO happy new year",
            "iso8601",
            "UTC",
            "UTC",
            0,
            "input",
            "timestamp",
            false,
            "auto",
            0.0,
            "keep",
            false,
        )
        .unwrap();
        assert_eq!(
            inferred,
            "2023-12-31T23:59:00+00:00\n2024-01-01T00:01:00+00:00"
        );

        // Nothing dated in the paste → the syslog line stays unmatched.
        assert_eq!(
            normalize("Dec  1 10:15:30 web01 sshd[1]: ok").unwrap(),
            "Dec  1 10:15:30 web01 sshd[1]: ok"
        );
    }

    #[test]
    fn output_mode_picks_what_each_line_is_made_of() {
        let line = "2023-12-01T10:15:30Z INFO boot";
        let with = |mode: &str| {
            run(
                line, "iso8601", "UTC", "UTC", 0, "input", mode, false, "auto", 0.0, "keep", false,
            )
            .unwrap()
        };
        assert_eq!(with("replace"), "2023-12-01T10:15:30+00:00 INFO boot");
        assert_eq!(
            with("prefix"),
            "2023-12-01T10:15:30+00:00  2023-12-01T10:15:30Z INFO boot"
        );
        assert_eq!(with("timestamp"), "2023-12-01T10:15:30+00:00");
    }

    #[test]
    fn sort_reorders_events_and_carries_their_trailing_lines() {
        let input =
            "2023-12-01T10:20:00Z ERROR boom\n  at Frame.one()\n2023-12-01T10:15:30Z INFO boot";
        let oldest = run(
            input, "iso8601", "UTC", "UTC", 0, "oldest", "replace", false, "auto", 0.0, "keep",
            false,
        )
        .unwrap();
        assert_eq!(
            oldest,
            "2023-12-01T10:15:30+00:00 INFO boot\n\
             2023-12-01T10:20:00+00:00 ERROR boom\n  at Frame.one()"
        );
        let newest = run(
            input, "iso8601", "UTC", "UTC", 0, "newest", "replace", false, "auto", 0.0, "keep",
            false,
        )
        .unwrap();
        assert!(newest.starts_with("2023-12-01T10:20:00+00:00 ERROR boom\n  at Frame.one()"));
    }

    #[test]
    fn delta_format_controls_the_annotation() {
        let input = "2023-12-01T10:15:30Z a\n2023-12-01T10:17:15.500Z b";
        let with = |fmt: &str| {
            run(
                input,
                "iso8601",
                "UTC",
                "UTC",
                0,
                "input",
                "timestamp",
                true,
                fmt,
                0.0,
                "keep",
                false,
            )
            .unwrap()
        };
        assert!(with("auto").ends_with("(+1m45s)"));
        assert!(with("seconds").ends_with("(+105.5s)"));
        assert!(with("milliseconds").ends_with("(+105500ms)"));
        assert!(with("hms").ends_with("(+0:01:45)"));
        assert!(with("auto").starts_with("2023-12-01T10:15:30+00:00  (start)"));
    }

    #[test]
    fn gap_threshold_marks_slow_steps_only() {
        let input = "2023-12-01T10:15:30Z a\n2023-12-01T10:15:31Z b\n2023-12-01T10:20:31Z c";
        let out = run(
            input,
            "iso8601",
            "UTC",
            "UTC",
            0,
            "input",
            "timestamp",
            true,
            "auto",
            60.0,
            "keep",
            false,
        )
        .unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[1], "2023-12-01T10:15:31+00:00  (+1s)");
        assert_eq!(lines[2], "2023-12-01T10:20:31+00:00  (+5m00s GAP)");
    }

    #[test]
    fn unmatched_lines_are_kept_dropped_or_marked() {
        let input = "== boot log ==\n2023-12-01T10:15:30Z INFO boot\n  at Frame.one()";
        let with = |policy: &str| {
            run(
                input, "iso8601", "UTC", "UTC", 0, "input", "replace", false, "auto", 0.0, policy,
                false,
            )
            .unwrap()
        };
        assert_eq!(
            with("keep"),
            "== boot log ==\n2023-12-01T10:15:30+00:00 INFO boot\n  at Frame.one()"
        );
        assert_eq!(with("drop"), "2023-12-01T10:15:30+00:00 INFO boot");
        assert_eq!(
            with("mark"),
            "== boot log ==  (no timestamp)\n\
             2023-12-01T10:15:30+00:00 INFO boot\n  at Frame.one()  (no timestamp)"
        );
    }

    #[test]
    fn summary_header_reports_the_format_mix_span_and_gaps() {
        let out = run(
            MIXED, "iso8601", "UTC", "UTC", 0, "input", "replace", false, "auto", 30.0, "keep",
            true,
        )
        .unwrap();
        let header: Vec<&str> = out.lines().take(6).collect();
        assert_eq!(
            header,
            vec![
                "# 5 lines · 5 timestamps · 0 without one",
                "# formats: apache 1, epoch-ms 1, epoch-s 1, iso-8601 1, syslog 1",
                "# span: 2023-12-01T10:15:30+00:00 → 2023-12-01T10:17:10+00:00 (1m40s)",
                "# largest gap: 45s at input line 4",
                "# gaps at or above 30s: 2",
                "# output: iso8601 in UTC",
            ]
        );
    }

    // -- errors ------------------------------------------------------------

    #[test]
    fn rejects_an_unknown_timezone() {
        let err = run(
            "2023-12-01T10:15:30Z x",
            "iso8601",
            "Mars/Olympus",
            "UTC",
            0,
            "input",
            "replace",
            true,
            "auto",
            0.0,
            "keep",
            false,
        )
        .unwrap_err();
        assert!(err.contains("unknown output_timezone"), "{err}");
        let err = run(
            "2023-12-01T10:15:30Z x",
            "iso8601",
            "UTC",
            "Nowhere/Land",
            0,
            "input",
            "replace",
            true,
            "auto",
            0.0,
            "keep",
            false,
        )
        .unwrap_err();
        assert!(err.contains("unknown assume_timezone"), "{err}");
    }

    #[test]
    fn rejects_unknown_enum_values_and_bad_numbers() {
        let bad = |field: &str| match field {
            "format" => run(
                "x", "sundial", "UTC", "UTC", 0, "input", "replace", true, "auto", 0.0, "keep",
                false,
            ),
            "sort" => run(
                "x", "iso8601", "UTC", "UTC", 0, "sideways", "replace", true, "auto", 0.0, "keep",
                false,
            ),
            "mode" => run(
                "x", "iso8601", "UTC", "UTC", 0, "input", "sideways", true, "auto", 0.0, "keep",
                false,
            ),
            "delta" => run(
                "x",
                "iso8601",
                "UTC",
                "UTC",
                0,
                "input",
                "replace",
                true,
                "fortnights",
                0.0,
                "keep",
                false,
            ),
            _ => run(
                "x", "iso8601", "UTC", "UTC", 0, "input", "replace", true, "auto", 0.0, "shred",
                false,
            ),
        };
        assert!(bad("format").unwrap_err().contains("unknown output_format"));
        assert!(bad("sort").unwrap_err().contains("unknown sort"));
        assert!(bad("mode").unwrap_err().contains("unknown output_mode"));
        assert!(bad("delta").unwrap_err().contains("unknown delta_format"));
        assert!(bad("unmatched").unwrap_err().contains("unknown unmatched"));

        let err = run(
            "x", "iso8601", "UTC", "UTC", 1200, "input", "replace", true, "auto", 0.0, "keep",
            false,
        )
        .unwrap_err();
        assert!(err.contains("assume_year"), "{err}");
        let err = run(
            "x", "iso8601", "UTC", "UTC", 0, "input", "replace", true, "auto", -5.0, "keep", false,
        )
        .unwrap_err();
        assert!(err.contains("gap_threshold_seconds"), "{err}");
    }

    #[test]
    fn rejects_empty_input_and_oversized_pastes() {
        assert!(normalize("   \n  ").unwrap_err().contains("no log text"));

        let at_cap = vec!["2023-12-01T10:15:30Z ok"; MAX_LINES].join("\n");
        assert!(normalize(&at_cap).is_ok());
        let over_cap = vec!["2023-12-01T10:15:30Z ok"; MAX_LINES + 1].join("\n");
        let err = normalize(&over_cap).unwrap_err();
        assert!(err.contains("too many"), "{err}");
        assert!(err.contains("50000"), "{err}");
    }

    #[test]
    fn a_trailing_newline_does_not_add_a_blank_line() {
        assert_eq!(
            normalize("2023-12-01T10:15:30Z ok\n").unwrap(),
            "2023-12-01T10:15:30+00:00 ok  (start)"
        );
        assert_eq!(
            normalize("2023-12-01T10:15:30Z ok\r\n").unwrap(),
            "2023-12-01T10:15:30+00:00 ok  (start)"
        );
    }
}
