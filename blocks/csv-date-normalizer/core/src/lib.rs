//! csv-date-normalizer core — pure compute, shared by the chat skill block and the web page.
//!
//! Rewrites the date/time values in one or more CSV columns into a single
//! canonical format (ISO 8601 by default). The hard part is not the rendering,
//! it is deciding what `03/04/2024` means: the parser reads every value in a
//! column first, uses the rows that can only be one thing (a day above 12) to
//! settle day-first vs month-first for the whole column, and only then rewrites.
//! Unparseable cells are never guessed at — they are kept, blanked, or reported,
//! whichever the caller asked for.
//!
//! Pure-Rust (`csv` + `chrono`); no wafer/wasm-bindgen deps, no clock, no I/O.

use chrono::format::{Item, StrftimeItems};
use chrono::{Duration, NaiveDate, NaiveDateTime, NaiveTime};
use serde::Serialize;

/// Hard cap on the input size, so a huge paste can't exhaust memory. The chat/CLI
/// schema and the page copy advertise this same bound.
pub const MAX_BYTES: usize = 5_000_000;

/// Most failing values listed per column in the report.
const FAILURE_CAP: usize = 20;

/// Fraction of a column's non-blank values that must parse as a written date
/// before auto-detection claims the column.
const AUTO_DETECT_RATIO: f64 = 0.6;

/// Highest Excel 1900-system serial (9999-12-31).
const MAX_EXCEL_SERIAL: f64 = 2_958_465.0;

// ---------------------------------------------------------------------------
// delimiters
// ---------------------------------------------------------------------------

/// Map a delimiter name (or a literal single char) to its byte. `auto` yields the
/// 0 sentinel, which the caller resolves by sniffing the first non-blank line.
fn delim_byte(d: &str) -> Result<u8, String> {
    Ok(match d.trim() {
        "" | "auto" => 0,
        "," | "comma" => b',',
        "\t" | "tab" | "\\t" => b'\t',
        ";" | "semicolon" => b';',
        "|" | "pipe" => b'|',
        other => {
            let b = other.as_bytes();
            if b.len() == 1 {
                b[0]
            } else {
                return Err(format!(
                    "delimiter must be auto/comma/tab/semicolon/pipe or a single character, got '{other}'"
                ));
            }
        }
    })
}

/// Sniff the delimiter from the first non-blank line: the candidate with the most
/// occurrences wins, ties broken by the preference order below, comma by default.
fn detect_delimiter(text: &str) -> u8 {
    let line = text.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
    let mut best = b',';
    let mut best_n = 0usize;
    for (byte, ch) in [(b',', ','), (b'\t', '\t'), (b';', ';'), (b'|', '|')] {
        let n = line.matches(ch).count();
        if n > best_n {
            best_n = n;
            best = byte;
        }
    }
    best
}

// ---------------------------------------------------------------------------
// parsing
// ---------------------------------------------------------------------------

/// Which component comes first in an all-numeric date whose parts are all ≤ 12.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DateOrder {
    /// Decide per column from the values that can only be read one way.
    Auto,
    /// `03/04/2024` is 4 March.
    DayFirst,
    /// `03/04/2024` is 3 April.
    MonthFirst,
}

fn parse_order(s: &str) -> Result<DateOrder, String> {
    Ok(match s.trim() {
        "" | "auto" => DateOrder::Auto,
        "day-first" | "dayfirst" | "dmy" => DateOrder::DayFirst,
        "month-first" | "monthfirst" | "mdy" => DateOrder::MonthFirst,
        other => {
            return Err(format!(
                "unknown date_order '{other}'; expected auto, day-first, or month-first"
            ))
        }
    })
}

fn order_label(o: DateOrder) -> &'static str {
    match o {
        DateOrder::Auto => "auto",
        DateOrder::DayFirst => "day-first",
        DateOrder::MonthFirst => "month-first",
    }
}

/// One successfully-read value.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Parsed {
    /// The wall-clock instant the cell described.
    dt: NaiveDateTime,
    /// The cell carried a time of day (so a date-only rendering would lose data).
    has_time: bool,
    /// UTC offset in seconds, when the cell stated one.
    offset: Option<i32>,
    /// The cell was a bare number (epoch, Excel serial, compact digits) rather
    /// than a written date. Auto column-detection ignores these — a column of
    /// plain integers is far more often an amount or an id than a date.
    numeric: bool,
    /// This value settles the day/month order for its column on its own.
    proves: Option<DateOrder>,
}

/// English month name or 3+ letter abbreviation → 1-12.
fn month_from_name(tok: &str) -> Option<u32> {
    const FULL: [&str; 12] = [
        "january",
        "february",
        "march",
        "april",
        "may",
        "june",
        "july",
        "august",
        "september",
        "october",
        "november",
        "december",
    ];
    let t = tok.trim_end_matches('.').to_ascii_lowercase();
    if t.len() < 3 {
        return None;
    }
    FULL.iter()
        .position(|f| f.starts_with(&t))
        .map(|i| i as u32 + 1)
}

/// True for a leading weekday token (`Tue`, `Tuesday`, RFC 2822 style).
fn is_weekday(tok: &str) -> bool {
    const DAYS: [&str; 7] = [
        "monday",
        "tuesday",
        "wednesday",
        "thursday",
        "friday",
        "saturday",
        "sunday",
    ];
    let t = tok.trim_end_matches('.').to_ascii_lowercase();
    t.len() >= 3 && DAYS.iter().any(|d| d.starts_with(&t))
}

/// Drop English ordinal suffixes that follow a number: `Jan 15th` → `Jan 15`.
fn strip_ordinals(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < chars.len() {
        if chars[i].is_ascii_digit() {
            let start = i;
            while i < chars.len() && chars[i].is_ascii_digit() {
                i += 1;
            }
            out.extend(chars[start..i].iter());
            if i + 1 < chars.len() {
                let suffix: String = chars[i..i + 2].iter().collect::<String>().to_ascii_lowercase();
                let ends_token = i + 2 >= chars.len() || !chars[i + 2].is_alphanumeric();
                if ends_token && matches!(suffix.as_str(), "st" | "nd" | "rd" | "th") {
                    i += 2;
                }
            }
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out
}

/// Expand a two-digit year against the pivot: `pivot` and below land in the
/// 2000s, above it in the 1900s (the same rule Excel and POSIX `%y` use).
fn expand_year(y: i32, pivot: i32) -> i32 {
    if y <= pivot {
        2000 + y
    } else {
        1900 + y
    }
}

/// `+05:30`, `-0500`, `+05` → offset in seconds east of UTC.
fn parse_offset(s: &str) -> Option<i32> {
    let bytes = s.as_bytes();
    let sign = match bytes.first()? {
        b'+' => 1,
        b'-' => -1,
        _ => return None,
    };
    let body: String = s[1..].chars().filter(|c| *c != ':').collect();
    if body.is_empty() || !body.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let (h, m) = match body.len() {
        2 => (body.parse::<i32>().ok()?, 0),
        4 => (
            body[..2].parse::<i32>().ok()?,
            body[2..].parse::<i32>().ok()?,
        ),
        _ => return None,
    };
    if h > 14 || m > 59 {
        return None;
    }
    Some(sign * (h * 3600 + m * 60))
}

/// Split a trailing UTC offset (`Z`, `+05:30`, `-0500`) off a time token.
fn split_offset(tok: &str) -> (&str, Option<i32>) {
    if let Some(rest) = tok.strip_suffix(['Z', 'z']) {
        if !rest.is_empty() {
            return (rest, Some(0));
        }
    }
    let bytes = tok.as_bytes();
    for i in (1..bytes.len()).rev() {
        if bytes[i] == b'+' || bytes[i] == b'-' {
            if let Some(off) = parse_offset(&tok[i..]) {
                return (&tok[..i], Some(off));
            }
        }
    }
    (tok, None)
}

/// `10:30`, `10:30:05`, `10:30:05.250`, `2:30pm` → a time of day.
fn parse_time_body(body: &str, mut meridiem: Option<bool>) -> Option<NaiveTime> {
    let mut body = body.trim();
    let lower = body.to_ascii_lowercase();
    for (suffix, pm) in [("am", false), ("pm", true)] {
        if lower.ends_with(suffix) {
            meridiem = Some(pm);
            body = body[..body.len() - 2].trim();
            break;
        }
    }
    let parts: Vec<&str> = body.split(':').collect();
    if parts.len() < 2 || parts.len() > 3 {
        return None;
    }
    let mut hour: u32 = parts[0].trim().parse().ok()?;
    let minute: u32 = parts[1].trim().parse().ok()?;
    let (second, nanos) = match parts.get(2) {
        None => (0u32, 0u32),
        Some(sec) => {
            let (whole, frac) = match sec.split_once('.') {
                Some((w, f)) => (w, f),
                None => (*sec, ""),
            };
            let s: u32 = whole.trim().parse().ok()?;
            let mut nanos = 0u32;
            if !frac.is_empty() {
                if !frac.chars().all(|c| c.is_ascii_digit()) {
                    return None;
                }
                let digits: String = frac.chars().take(9).collect();
                let scale = 10u32.pow(9 - digits.len() as u32);
                nanos = digits.parse::<u32>().ok()? * scale;
            }
            (s, nanos)
        }
    };
    if let Some(pm) = meridiem {
        if hour == 0 || hour > 12 {
            return None;
        }
        hour = match (hour, pm) {
            (12, false) => 0,
            (12, true) => 12,
            (h, false) => h,
            (h, true) => h + 12,
        };
    }
    NaiveTime::from_hms_nano_opt(hour, minute, second, nanos)
}

/// Read the three date components once they have been split into text parts.
/// Returns the date plus the order it proves, if the values settle it alone.
fn parse_date_parts(
    parts: &[&str],
    order: DateOrder,
    pivot: i32,
) -> Option<(NaiveDate, Option<DateOrder>)> {
    if parts.len() != 3 {
        return None;
    }
    let months: Vec<Option<u32>> = parts.iter().map(|p| month_from_name(p)).collect();
    let named = months.iter().filter(|m| m.is_some()).count();

    if named == 1 {
        // "15 Jan 2024" / "Jan 15, 2024" / "2024 Jan 15" — the month is unambiguous,
        // the year is the four-digit number (or the last one if both are short).
        let month = months.iter().flatten().next().copied()?;
        let nums: Vec<&str> = parts
            .iter()
            .zip(&months)
            .filter(|(_, m)| m.is_none())
            .map(|(p, _)| *p)
            .collect();
        if nums.len() != 2 {
            return None;
        }
        let (year_tok, day_tok) = if nums[0].len() == 4 {
            (nums[0], nums[1])
        } else {
            (nums[1], nums[0])
        };
        let year = parse_year(year_tok, pivot)?;
        let day: u32 = numeric_part(day_tok)?;
        return NaiveDate::from_ymd_opt(year, month, day).map(|d| (d, None));
    }
    if named > 0 {
        return None;
    }

    // All numeric.
    if parts[0].len() == 4 {
        let year = parse_year(parts[0], pivot)?;
        let month = numeric_part(parts[1])?;
        let day = numeric_part(parts[2])?;
        return NaiveDate::from_ymd_opt(year, month, day).map(|d| (d, None));
    }
    let year = parse_year(parts[2], pivot)?;
    let a = numeric_part(parts[0])?;
    let b = numeric_part(parts[1])?;
    let (day, month, proves) = if a > 12 && b <= 12 {
        (a, b, Some(DateOrder::DayFirst))
    } else if b > 12 && a <= 12 {
        (b, a, Some(DateOrder::MonthFirst))
    } else if a > 12 && b > 12 {
        return None;
    } else {
        match order {
            DateOrder::DayFirst => (a, b, None),
            // Auto with no evidence falls back to month-first; the report says so.
            _ => (b, a, None),
        }
    };
    NaiveDate::from_ymd_opt(year, month, day).map(|d| (d, proves))
}

/// A 1-2 digit (or longer) numeric component with no sign or decimal point.
fn numeric_part(tok: &str) -> Option<u32> {
    let t = tok.trim();
    if t.is_empty() || !t.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    t.parse().ok()
}

/// A year component: four digits verbatim, one or two digits via the pivot.
fn parse_year(tok: &str, pivot: i32) -> Option<i32> {
    let t = tok.trim();
    if t.is_empty() || !t.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let n: i32 = t.parse().ok()?;
    Some(if t.len() <= 2 { expand_year(n, pivot) } else { n })
}

/// Excel 1900-system serial → date-time. Day 1 is 1900-01-01, and the sheet's
/// phantom 1900-02-29 is why the epoch below is 1899-12-30.
fn from_excel_serial(serial: f64) -> Option<(NaiveDateTime, bool)> {
    let base = NaiveDate::from_ymd_opt(1899, 12, 30)?;
    let days = serial.trunc();
    let frac = serial - days;
    let date = base.checked_add_signed(Duration::days(days as i64))?;
    let seconds = (frac * 86_400.0).round() as i64;
    let dt = date.and_hms_opt(0, 0, 0)? + Duration::seconds(seconds);
    Some((dt, seconds != 0))
}

/// Bare-number branch: compact digits, Unix epoch, or an Excel serial.
fn parse_numeric(s: &str, excel_serial: bool) -> Option<Parsed> {
    let digits_only = !s.is_empty() && s.chars().all(|c| c.is_ascii_digit());
    if digits_only && s.len() == 8 {
        let year = s[..4].parse::<i32>().ok()?;
        let month = s[4..6].parse::<u32>().ok()?;
        let day = s[6..8].parse::<u32>().ok()?;
        let dt = NaiveDate::from_ymd_opt(year, month, day)?.and_hms_opt(0, 0, 0)?;
        return Some(Parsed { dt, has_time: false, offset: None, numeric: true, proves: None });
    }
    if digits_only && s.len() == 14 {
        let year = s[..4].parse::<i32>().ok()?;
        let month = s[4..6].parse::<u32>().ok()?;
        let day = s[6..8].parse::<u32>().ok()?;
        let hour = s[8..10].parse::<u32>().ok()?;
        let minute = s[10..12].parse::<u32>().ok()?;
        let second = s[12..14].parse::<u32>().ok()?;
        let dt = NaiveDate::from_ymd_opt(year, month, day)?.and_hms_opt(hour, minute, second)?;
        return Some(Parsed { dt, has_time: true, offset: None, numeric: true, proves: None });
    }

    // A signed/decimal number: Excel serial, epoch seconds, or epoch milliseconds.
    let n: f64 = s.parse().ok()?;
    if !n.is_finite() {
        return None;
    }
    let mag = n.abs();
    if excel_serial && n >= 1.0 && n <= MAX_EXCEL_SERIAL {
        let (dt, has_time) = from_excel_serial(n)?;
        return Some(Parsed { dt, has_time, offset: None, numeric: true, proves: None });
    }
    let (secs, nanos, has_time) = if (1e8..1e11).contains(&mag) {
        let secs = n.trunc() as i64;
        (secs, ((n - n.trunc()) * 1e9).round() as u32, true)
    } else if (1e11..1e14).contains(&mag) {
        let ms = n.trunc() as i64;
        (ms.div_euclid(1000), (ms.rem_euclid(1000) * 1_000_000) as u32, true)
    } else {
        return None;
    };
    let dt = chrono::DateTime::from_timestamp(secs, nanos)?.naive_utc();
    Some(Parsed { dt, has_time, offset: Some(0), numeric: true, proves: None })
}

/// Read one cell. `order` only matters for all-numeric dates whose day and month
/// are both ≤ 12; every other shape settles itself.
fn parse_cell(raw: &str, order: DateOrder, pivot: i32, excel_serial: bool) -> Option<Parsed> {
    let s = raw.trim();
    if s.is_empty() {
        return None;
    }
    if let Some(p) = parse_numeric(s, excel_serial) {
        return Some(p);
    }

    // Written date: normalize the separators the CSV world actually uses.
    let mut normalized = strip_ordinals(s).replace(',', " ");
    // ISO's `T` joins the date and time with no space around it.
    let bytes: Vec<char> = normalized.chars().collect();
    let mut rebuilt = String::with_capacity(normalized.len());
    for (i, c) in bytes.iter().enumerate() {
        let joins_date_and_time = (*c == 'T' || *c == 't')
            && i > 0
            && bytes[i - 1].is_ascii_digit()
            && bytes.get(i + 1).is_some_and(|n| n.is_ascii_digit());
        rebuilt.push(if joins_date_and_time { ' ' } else { *c });
    }
    normalized = rebuilt;

    let mut tokens: Vec<&str> = normalized.split_whitespace().collect();
    if tokens.is_empty() {
        return None;
    }
    if is_weekday(tokens[0]) && tokens.len() > 1 {
        tokens.remove(0);
    }

    let time_at = tokens.iter().position(|t| t.contains(':'));
    let (date_tokens, time_token, zone_tokens): (&[&str], Option<&str>, &[&str]) = match time_at {
        Some(i) => (&tokens[..i], Some(tokens[i]), &tokens[i + 1..]),
        None => (&tokens[..], None, &[]),
    };
    if date_tokens.is_empty() {
        return None;
    }

    let parts: Vec<&str> = if date_tokens.len() == 1 {
        date_tokens[0]
            .split(['/', '-', '.'])
            .filter(|p| !p.is_empty())
            .collect()
    } else {
        date_tokens.to_vec()
    };
    let (date, proves) = parse_date_parts(&parts, order, pivot)?;

    // Trailing tokens: a meridiem, a named zone, or a numeric offset.
    let mut offset: Option<i32> = None;
    let mut meridiem: Option<bool> = None;
    for tok in zone_tokens {
        let lower = tok.to_ascii_lowercase();
        match lower.as_str() {
            "am" | "a.m." => meridiem = Some(false),
            "pm" | "p.m." => meridiem = Some(true),
            "z" | "utc" | "gmt" | "ut" => offset = Some(0),
            _ => match parse_offset(tok) {
                Some(o) => offset = Some(o),
                // An unrecognized trailing token means we did not really
                // understand the value; refuse rather than silently drop it.
                None => return None,
            },
        }
    }

    let (time, has_time) = match time_token {
        None => (NaiveTime::from_hms_opt(0, 0, 0)?, false),
        Some(tok) => {
            let (body, tok_offset) = split_offset(tok);
            if tok_offset.is_some() {
                offset = tok_offset;
            }
            (parse_time_body(body, meridiem)?, true)
        }
    };
    if time_token.is_none() && meridiem.is_some() {
        return None;
    }

    Some(Parsed { dt: date.and_time(time), has_time, offset, numeric: false, proves })
}

// ---------------------------------------------------------------------------
// rendering
// ---------------------------------------------------------------------------

/// `0` → `Z`, otherwise `+HH:MM` / `-HH:MM`.
fn format_offset(secs: i32) -> String {
    if secs == 0 {
        return "Z".to_string();
    }
    let sign = if secs < 0 { '-' } else { '+' };
    let abs = secs.abs();
    format!("{sign}{:02}:{:02}", abs / 3600, (abs % 3600) / 60)
}

/// Shift a wall-clock time at `offset` to the same instant in UTC.
fn to_utc(p: &Parsed) -> NaiveDateTime {
    p.dt - Duration::seconds(p.offset.unwrap_or(0) as i64)
}

fn check_custom_pattern(fmt: &str) -> Result<(), String> {
    if fmt.trim().is_empty() {
        return Err("custom_format is required when format is 'custom' (for example %d %B %Y)".into());
    }
    if StrftimeItems::new(fmt).any(|item| matches!(item, Item::Error)) {
        return Err(format!(
            "invalid custom_format '{fmt}'; use chrono/strftime specifiers such as %Y %m %d %H %M %S %b %B"
        ));
    }
    Ok(())
}

/// Render one parsed value into the chosen output format.
fn render(p: &Parsed, format: &str, custom: &str) -> String {
    match format {
        "iso-auto" => {
            if p.has_time {
                let base = p.dt.format("%Y-%m-%dT%H:%M:%S").to_string();
                match p.offset {
                    Some(o) => format!("{base}{}", format_offset(o)),
                    None => base,
                }
            } else {
                p.dt.format("%Y-%m-%d").to_string()
            }
        }
        "iso-date" => p.dt.format("%Y-%m-%d").to_string(),
        "iso-datetime" => p.dt.format("%Y-%m-%dT%H:%M:%S").to_string(),
        "iso-utc" => to_utc(p).format("%Y-%m-%dT%H:%M:%SZ").to_string(),
        "unix-seconds" => to_utc(p).and_utc().timestamp().to_string(),
        "unix-millis" => to_utc(p).and_utc().timestamp_millis().to_string(),
        "us-date" => p.dt.format("%m/%d/%Y").to_string(),
        "eu-date" => p.dt.format("%d/%m/%Y").to_string(),
        "sql" => p.dt.format("%Y-%m-%d %H:%M:%S").to_string(),
        "compact" => p.dt.format("%Y%m%d").to_string(),
        "rfc2822" => {
            let offset = p.offset.unwrap_or(0);
            let sign = if offset < 0 { '-' } else { '+' };
            let abs = offset.abs();
            format!(
                "{} {sign}{:02}{:02}",
                p.dt.format("%a, %d %b %Y %H:%M:%S"),
                abs / 3600,
                (abs % 3600) / 60
            )
        }
        // Already validated by check_custom_pattern.
        _ => p.dt.format(custom).to_string(),
    }
}

const FORMATS: [&str; 12] = [
    "iso-auto",
    "iso-date",
    "iso-datetime",
    "iso-utc",
    "unix-seconds",
    "unix-millis",
    "us-date",
    "eu-date",
    "sql",
    "compact",
    "rfc2822",
    "custom",
];

// ---------------------------------------------------------------------------
// report
// ---------------------------------------------------------------------------

/// One value the parser refused.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Failure {
    /// 1-based line number in the original CSV text.
    pub line: usize,
    /// 1-based index among DATA rows only (header excluded).
    pub row: usize,
    /// The value that could not be read.
    pub value: String,
}

/// What happened to one normalized column.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ColumnReport {
    /// Header name, or `col{n}` for headerless input.
    pub column: String,
    /// 0-based column index.
    pub index: usize,
    /// The day/month order used for this column's all-numeric dates.
    pub date_order: String,
    /// Why that order was chosen: `explicit`, `detected`, `default`, or `conflict`.
    pub order_source: String,
    /// Non-blank cells seen in the column.
    pub values: usize,
    /// Cells rewritten into the target format.
    pub converted: usize,
    /// Cells that were already identical to their normalized form.
    pub already_normalized: usize,
    /// Cells the parser refused.
    pub failed: usize,
    /// Blank cells, always passed through untouched.
    pub blank: usize,
    /// The refused values, in document order, capped at 20.
    pub failures: Vec<Failure>,
}

/// The whole run.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Report {
    /// The target format the values were rewritten into.
    pub format: String,
    /// Data rows processed (header excluded).
    pub rows: usize,
    /// Per-column detail, in column order.
    pub columns: Vec<ColumnReport>,
    /// Cells rewritten across all columns.
    pub converted_total: usize,
    /// Cells refused across all columns.
    pub failed_total: usize,
    /// The rewritten CSV.
    pub csv: String,
}

// ---------------------------------------------------------------------------
// column selection
// ---------------------------------------------------------------------------

fn resolve_columns(
    spec: &str,
    columns: &[String],
    has_header: bool,
    rows: &[(usize, csv::StringRecord)],
    pivot: i32,
    excel_serial: bool,
) -> Result<Vec<usize>, String> {
    let spec = spec.trim();
    if spec.is_empty() || spec == "auto" {
        let mut found = Vec::new();
        for (i, _) in columns.iter().enumerate() {
            let mut non_blank = 0usize;
            let mut written_dates = 0usize;
            for (_, rec) in rows {
                let cell = rec.get(i).unwrap_or("").trim();
                if cell.is_empty() {
                    continue;
                }
                non_blank += 1;
                if parse_cell(cell, DateOrder::MonthFirst, pivot, excel_serial)
                    .is_some_and(|p| !p.numeric)
                {
                    written_dates += 1;
                }
            }
            if non_blank > 0 && (written_dates as f64) >= AUTO_DETECT_RATIO * non_blank as f64 {
                found.push(i);
            }
        }
        if found.is_empty() {
            return Err(
                "no date columns detected; name the columns explicitly (a header name or a 0-based index). Auto-detection ignores columns of bare numbers, so Unix timestamps and Excel serials must be named."
                    .into(),
            );
        }
        return Ok(found);
    }

    let mut out = Vec::new();
    for part in spec.split(',') {
        let name = part.trim();
        if name.is_empty() {
            continue;
        }
        let idx = if let Ok(n) = name.parse::<usize>() {
            if n >= columns.len() {
                return Err(format!(
                    "column index {n} out of range (0..={})",
                    columns.len().saturating_sub(1)
                ));
            }
            n
        } else if !has_header {
            return Err(format!(
                "with 'First row is a header' off, columns must be 0-based indexes, got '{name}'"
            ));
        } else {
            columns
                .iter()
                .position(|c| c == name)
                .ok_or_else(|| format!("column '{name}' not found in header ({})", columns.join(", ")))?
        };
        if !out.contains(&idx) {
            out.push(idx);
        }
    }
    if out.is_empty() {
        return Err("columns is empty; give a header name, a 0-based index, or 'auto'".into());
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// entry point
// ---------------------------------------------------------------------------

/// Normalize the date values in the chosen CSV columns.
#[allow(clippy::too_many_arguments)]
pub fn run(
    data: &str,
    columns_spec: &str,
    format: &str,
    custom_format: &str,
    date_order: &str,
    year_pivot: i64,
    excel_serial: bool,
    on_error: &str,
    has_header: bool,
    delimiter: &str,
    output: &str,
) -> Result<String, String> {
    if data.trim().is_empty() {
        return Err("input is empty".into());
    }
    if data.len() > MAX_BYTES {
        return Err(format!(
            "input is {} bytes; the limit is {MAX_BYTES} bytes",
            data.len()
        ));
    }
    let format = match format.trim() {
        "" => "iso-auto",
        f if FORMATS.contains(&f) => f,
        other => {
            return Err(format!(
                "unknown format '{other}'; expected one of {}",
                FORMATS.join(", ")
            ))
        }
    };
    if format == "custom" {
        check_custom_pattern(custom_format)?;
    }
    let on_error = match on_error.trim() {
        "" => "keep",
        v @ ("keep" | "blank" | "error") => v,
        other => {
            return Err(format!(
                "unknown on_error '{other}'; expected keep, blank, or error"
            ))
        }
    };
    let output = match output.trim() {
        "" => "csv",
        v @ ("csv" | "report" | "json") => v,
        other => {
            return Err(format!(
                "unknown output '{other}'; expected csv, report, or json"
            ))
        }
    };
    if !(0..=99).contains(&year_pivot) {
        return Err(format!(
            "year_pivot must be between 0 and 99, got {year_pivot}"
        ));
    }
    let pivot = year_pivot as i32;
    let requested_order = parse_order(date_order)?;

    let delim = match delim_byte(delimiter)? {
        0 => detect_delimiter(data),
        b => b,
    };

    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(delim)
        .has_headers(false)
        .flexible(true)
        .from_reader(data.as_bytes());

    let mut records: Vec<(usize, csv::StringRecord)> = Vec::new();
    let mut rec = csv::StringRecord::new();
    loop {
        match rdr.read_record(&mut rec) {
            Ok(true) => {
                let line = rec.position().map(|p| p.line() as usize).unwrap_or(0);
                records.push((line, rec.clone()));
            }
            Ok(false) => break,
            Err(e) => return Err(format!("CSV parse error: {e}")),
        }
    }
    if records.is_empty() {
        return Err("no rows found".into());
    }

    let width = records.iter().map(|(_, r)| r.len()).max().unwrap_or(0);
    let header = &records[0].1;
    let column_names: Vec<String> = (0..width)
        .map(|i| {
            let name = if has_header { header.get(i).unwrap_or("").trim() } else { "" };
            if name.is_empty() {
                format!("col{}", i + 1)
            } else {
                name.to_string()
            }
        })
        .collect();

    let data_rows: Vec<(usize, csv::StringRecord)> =
        if has_header { records[1..].to_vec() } else { records.clone() };
    if data_rows.is_empty() {
        return Err("no data rows found (the input has only a header row)".into());
    }

    let targets = resolve_columns(
        columns_spec,
        &column_names,
        has_header,
        &data_rows,
        pivot,
        excel_serial,
    )?;

    // Pass 1 — settle day-first vs month-first per column from the values that
    // can only be read one way.
    let mut orders: Vec<(DateOrder, &'static str)> = Vec::with_capacity(targets.len());
    for &col in &targets {
        if requested_order != DateOrder::Auto {
            orders.push((requested_order, "explicit"));
            continue;
        }
        let mut day_votes = 0usize;
        let mut month_votes = 0usize;
        for (_, rec) in &data_rows {
            let cell = rec.get(col).unwrap_or("").trim();
            if cell.is_empty() {
                continue;
            }
            match parse_cell(cell, DateOrder::MonthFirst, pivot, excel_serial).and_then(|p| p.proves)
            {
                Some(DateOrder::DayFirst) => day_votes += 1,
                Some(DateOrder::MonthFirst) => month_votes += 1,
                _ => {}
            }
        }
        orders.push(match (day_votes, month_votes) {
            (0, 0) => (DateOrder::MonthFirst, "default"),
            (d, 0) if d > 0 => (DateOrder::DayFirst, "detected"),
            (0, _) => (DateOrder::MonthFirst, "detected"),
            // Both orders are proven in the same column — the data is genuinely
            // mixed. Fall back to month-first and flag it loudly in the report.
            _ => (DateOrder::MonthFirst, "conflict"),
        });
    }

    // Pass 2 — rewrite.
    let mut rows_out: Vec<Vec<String>> = Vec::with_capacity(data_rows.len());
    let mut reports: Vec<ColumnReport> = targets
        .iter()
        .zip(&orders)
        .map(|(&col, (order, source))| ColumnReport {
            column: column_names[col].clone(),
            index: col,
            date_order: order_label(*order).to_string(),
            order_source: (*source).to_string(),
            values: 0,
            converted: 0,
            already_normalized: 0,
            failed: 0,
            blank: 0,
            failures: Vec::new(),
        })
        .collect();

    for (row_i, (line, rec)) in data_rows.iter().enumerate() {
        let mut fields: Vec<String> = rec.iter().map(|f| f.to_string()).collect();
        for (slot, (&col, (order, _))) in targets.iter().zip(&orders).enumerate() {
            let Some(original) = fields.get(col).cloned() else { continue };
            let trimmed = original.trim();
            let report = &mut reports[slot];
            if trimmed.is_empty() {
                report.blank += 1;
                continue;
            }
            report.values += 1;
            match parse_cell(trimmed, *order, pivot, excel_serial) {
                Some(parsed) => {
                    let rendered = render(&parsed, format, custom_format);
                    if rendered == original {
                        report.already_normalized += 1;
                    } else {
                        report.converted += 1;
                    }
                    fields[col] = rendered;
                }
                None => {
                    report.failed += 1;
                    if report.failures.len() < FAILURE_CAP {
                        report.failures.push(Failure {
                            line: *line,
                            row: row_i + 1,
                            value: trimmed.to_string(),
                        });
                    }
                    match on_error {
                        "blank" => fields[col] = String::new(),
                        "error" => {
                            return Err(format!(
                                "row {} (line {line}), column '{}': cannot read '{trimmed}' as a date. Set 'Unreadable values' to keep or blank to continue past it.",
                                row_i + 1,
                                column_names[col]
                            ))
                        }
                        _ => {}
                    }
                }
            }
        }
        rows_out.push(fields);
    }

    let mut wtr = csv::WriterBuilder::new()
        .delimiter(delim)
        .flexible(true)
        .from_writer(Vec::new());
    if has_header {
        wtr.write_record(header.iter())
            .map_err(|e| format!("CSV write error: {e}"))?;
    }
    for fields in &rows_out {
        wtr.write_record(fields)
            .map_err(|e| format!("CSV write error: {e}"))?;
    }
    let csv_out = String::from_utf8(
        wtr.into_inner()
            .map_err(|e| format!("CSV write error: {e}"))?,
    )
    .map_err(|e| format!("CSV write error: {e}"))?;

    let report = Report {
        format: format.to_string(),
        rows: data_rows.len(),
        converted_total: reports.iter().map(|r| r.converted).sum(),
        failed_total: reports.iter().map(|r| r.failed).sum(),
        columns: reports,
        csv: csv_out,
    };

    match output {
        "csv" => Ok(report.csv),
        "json" => serde_json::to_string_pretty(&report).map_err(|e| e.to_string()),
        _ => Ok(render_report(&report)),
    }
}

fn render_report(r: &Report) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "Normalized {} column(s) to {} across {} data row(s).\n",
        r.columns.len(),
        r.format,
        r.rows
    ));
    out.push_str(&format!(
        "Converted: {}   Unreadable: {}\n",
        r.converted_total, r.failed_total
    ));
    for c in &r.columns {
        out.push_str(&format!(
            "\nColumn \"{}\" (index {}) — day/month order {} ({})\n",
            c.column, c.index, c.date_order, c.order_source
        ));
        if c.order_source == "conflict" {
            out.push_str("  ! this column proves BOTH orders; month-first was used. Set the day/month order explicitly.\n");
        }
        out.push_str(&format!(
            "  values {}, converted {}, already normalized {}, unreadable {}, blank {}\n",
            c.values, c.converted, c.already_normalized, c.failed, c.blank
        ));
        for f in &c.failures {
            out.push_str(&format!(
                "    row {} (line {}): \"{}\"\n",
                f.row, f.line, f.value
            ));
        }
        if c.failed > c.failures.len() {
            out.push_str(&format!(
                "    … {} more unreadable value(s) not listed\n",
                c.failed - c.failures.len()
            ));
        }
    }
    while out.ends_with('\n') {
        out.pop();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIXED: &str = "id,joined\n1,2021-06-01\n2,06/15/2021\n3,15 Jan 2024\n4,not a date";

    fn norm(data: &str, columns: &str) -> String {
        run(data, columns, "iso-auto", "", "auto", 68, true, "keep", true, "auto", "csv").unwrap()
    }

    #[test]
    fn happy_mixed_formats_to_iso() {
        let out = norm(MIXED, "joined");
        assert_eq!(
            out,
            "id,joined\n1,2021-06-01\n2,2021-06-15\n3,2024-01-15\n4,not a date\n"
        );
    }

    #[test]
    fn auto_detects_the_date_column() {
        let out = norm("id,joined,amount\n1,06/15/2021,100\n2,2021-07-04,200", "auto");
        assert_eq!(out, "id,joined,amount\n1,2021-06-15,100\n2,2021-07-04,200\n");
    }

    #[test]
    fn auto_detection_never_claims_a_number_column() {
        // 1700000000 is a valid Unix timestamp and 45000 a valid Excel serial, but a
        // column of bare numbers is an amount far more often than a date: the written
        // dates in `d` are claimed and `amount` is left exactly as it came in.
        let out = norm("d,amount\n06/15/2021,1700000000\n07/04/2021,45000", "auto");
        assert_eq!(out, "d,amount\n2021-06-15,1700000000\n2021-07-04,45000\n");
    }

    #[test]
    fn day_first_inferred_from_a_day_above_twelve() {
        // 25/12/2021 can only be day-first, so 03/04/2021 in the same column is 3 April.
        let out = norm("d\n25/12/2021\n03/04/2021", "d");
        assert_eq!(out, "d\n2021-12-25\n2021-04-03\n");
    }

    #[test]
    fn month_first_inferred_from_a_month_above_twelve() {
        let out = norm("d\n12/25/2021\n03/04/2021", "d");
        assert_eq!(out, "d\n2021-12-25\n2021-03-04\n");
    }

    #[test]
    fn ambiguous_column_defaults_to_month_first_and_says_so() {
        let out = run("d\n03/04/2021", "d", "iso-date", "", "auto", 68, true, "keep", true, "auto", "json").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["columns"][0]["date_order"], "month-first");
        assert_eq!(v["columns"][0]["order_source"], "default");
        assert_eq!(v["csv"], "d\n2021-03-04\n");
    }

    #[test]
    fn explicit_day_first_overrides_inference() {
        let out = run("d\n03/04/2021", "d", "iso-date", "", "day-first", 68, true, "keep", true, "auto", "csv").unwrap();
        assert_eq!(out, "d\n2021-04-03\n");
    }

    #[test]
    fn conflicting_orders_are_reported() {
        let out = run("d\n25/12/2021\n12/25/2021", "d", "iso-date", "", "auto", 68, true, "keep", true, "auto", "json").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["columns"][0]["order_source"], "conflict");
    }

    #[test]
    fn written_month_names_and_ordinals() {
        // The first value carries a comma, so in a real CSV it arrives quoted.
        let out = norm(
            "d\n\"Jan 15th, 2024\"\n15 January 2024\n01-Jun-2021\nSept. 3 1999",
            "d",
        );
        assert_eq!(out, "d\n2024-01-15\n2024-01-15\n2021-06-01\n1999-09-03\n");
    }

    #[test]
    fn two_digit_years_use_the_pivot() {
        let out = run("d\n01/02/99\n01/02/24", "d", "iso-date", "", "month-first", 68, true, "keep", true, "auto", "csv").unwrap();
        assert_eq!(out, "d\n1999-01-02\n2024-01-02\n");
        let shifted = run("d\n01/02/99", "d", "iso-date", "", "month-first", 99, true, "keep", true, "auto", "csv").unwrap();
        assert_eq!(shifted, "d\n2099-01-02\n");
    }

    #[test]
    fn excel_serials_and_unix_epochs() {
        let out = run(
            "d\n45000\n1700000000\n1700000000000\n20240115",
            "d",
            "iso-date",
            "",
            "auto",
            68,
            true,
            "keep",
            true,
            "auto",
            "csv",
        )
        .unwrap();
        assert_eq!(out, "d\n2023-03-15\n2023-11-14\n2023-11-14\n2024-01-15\n");
    }

    #[test]
    fn excel_serial_can_be_turned_off() {
        let out = run("d\n45000", "d", "iso-date", "", "auto", 68, false, "keep", true, "auto", "csv").unwrap();
        assert_eq!(out, "d\n45000\n");
    }

    #[test]
    fn times_offsets_and_meridiem() {
        let out = norm(
            // RFC 2822 puts a comma after the weekday, so the cell arrives quoted.
            "t\n2021-06-01T12:30:00Z\n2021-06-01 12:30:00+05:30\n3/4/2024 2:30 PM\n\"Tue, 15 Jan 2024 10:30:00 +0000\"",
            "t",
        );
        assert_eq!(
            out,
            "t\n2021-06-01T12:30:00Z\n2021-06-01T12:30:00+05:30\n2024-03-04T14:30:00\n2024-01-15T10:30:00Z\n"
        );
    }

    #[test]
    fn iso_utc_shifts_the_offset() {
        let out = run(
            "t\n2021-06-01 12:30:00+05:30\n2021-06-01T00:30:00-05:00",
            "t",
            "iso-utc",
            "",
            "auto",
            68,
            true,
            "keep",
            true,
            "auto",
            "csv",
        )
        .unwrap();
        assert_eq!(out, "t\n2021-06-01T07:00:00Z\n2021-06-01T05:30:00Z\n");
    }

    #[test]
    fn every_output_format_renders() {
        let expected = [
            ("iso-auto", "2021-06-01T12:30:00Z"),
            ("iso-date", "2021-06-01"),
            ("iso-datetime", "2021-06-01T12:30:00"),
            ("iso-utc", "2021-06-01T12:30:00Z"),
            ("unix-seconds", "1622550600"),
            ("unix-millis", "1622550600000"),
            ("us-date", "06/01/2021"),
            ("eu-date", "01/06/2021"),
            ("sql", "2021-06-01 12:30:00"),
            ("compact", "20210601"),
            ("rfc2822", "Tue, 01 Jun 2021 12:30:00 +0000"),
        ];
        for (fmt, want) in expected {
            let out = run("t\n2021-06-01T12:30:00Z", "t", fmt, "", "auto", 68, true, "keep", true, "auto", "csv").unwrap();
            assert_eq!(out, format!("t\n{}\n", quote_if_needed(want)), "format {fmt}");
        }
    }

    /// The CSV writer quotes any rendered value that contains the delimiter.
    fn quote_if_needed(v: &str) -> String {
        if v.contains(',') {
            format!("\"{v}\"")
        } else {
            v.to_string()
        }
    }

    #[test]
    fn custom_strftime_pattern() {
        let out = run("d\n2021-06-01", "d", "custom", "%d %B %Y", "auto", 68, true, "keep", true, "auto", "csv").unwrap();
        assert_eq!(out, "d\n01 June 2021\n");
    }

    #[test]
    fn multiple_columns_and_indexes() {
        let out = run(
            "a,start,end\n1,06/15/2021,20210704",
            "start,2",
            "iso-date",
            "",
            "auto",
            68,
            true,
            "keep",
            true,
            "auto",
            "csv",
        )
        .unwrap();
        assert_eq!(out, "a,start,end\n1,2021-06-15,2021-07-04\n");
    }

    #[test]
    fn headerless_input_uses_indexes() {
        let out = run("06/15/2021,x\n2021-07-04,y", "0", "iso-date", "", "auto", 68, true, "keep", false, "auto", "csv").unwrap();
        assert_eq!(out, "2021-06-15,x\n2021-07-04,y\n");
    }

    #[test]
    fn unreadable_values_can_be_blanked() {
        let out = run(MIXED, "joined", "iso-date", "", "auto", 68, true, "blank", true, "auto", "csv").unwrap();
        assert!(out.ends_with("4,\n"), "{out}");
    }

    #[test]
    fn semicolon_delimiter_round_trips() {
        let out = norm("id;d\n1;06/15/2021", "d");
        assert_eq!(out, "id;d\n1;2021-06-15\n");
    }

    #[test]
    fn quoted_fields_and_blanks_survive() {
        let out = norm("d,note\n06/15/2021,\"hello, world\"\n,\"kept\"", "d");
        assert_eq!(out, "d,note\n2021-06-15,\"hello, world\"\n,kept\n");
    }

    #[test]
    fn text_report_counts_everything() {
        let out = run(MIXED, "joined", "iso-date", "", "auto", 68, true, "keep", true, "auto", "report").unwrap();
        // 2021-06-01 was already ISO, so it counts as already-normalized rather
        // than converted — the report keeps the two apart on purpose.
        assert!(out.contains("Converted: 2   Unreadable: 1"), "{out}");
        assert!(
            out.contains("values 4, converted 2, already normalized 1, unreadable 1, blank 0"),
            "{out}"
        );
        assert!(out.contains("row 4 (line 5): \"not a date\""), "{out}");
    }

    #[test]
    fn impossible_calendar_dates_are_refused() {
        let out = norm("d\n2021-02-30\n2021-13-01\n2020-02-29", "d");
        assert_eq!(out, "d\n2021-02-30\n2021-13-01\n2020-02-29\n");
    }

    // --- error paths ---

    #[test]
    fn empty_input_errors() {
        assert!(run("", "d", "iso-date", "", "auto", 68, true, "keep", true, "auto", "csv").is_err());
    }

    #[test]
    fn on_error_error_stops_with_the_offending_cell() {
        let err = run(MIXED, "joined", "iso-date", "", "auto", 68, true, "error", true, "auto", "csv").unwrap_err();
        assert!(err.contains("cannot read 'not a date' as a date"), "{err}");
    }

    #[test]
    fn unknown_column_errors() {
        let err = run(MIXED, "nope", "iso-date", "", "auto", 68, true, "keep", true, "auto", "csv").unwrap_err();
        assert!(err.contains("not found in header"), "{err}");
    }

    #[test]
    fn column_index_out_of_range_errors() {
        let err = run("a,b\n1,2", "9", "iso-date", "", "auto", 68, true, "keep", true, "auto", "csv").unwrap_err();
        assert!(err.contains("out of range"), "{err}");
    }

    #[test]
    fn auto_detect_with_no_date_column_errors() {
        let err = run("a,b\n1,2\n3,4", "auto", "iso-date", "", "auto", 68, true, "keep", true, "auto", "csv").unwrap_err();
        assert!(err.contains("no date columns detected"), "{err}");
    }

    #[test]
    fn unknown_format_errors() {
        let err = run(MIXED, "joined", "julian", "", "auto", 68, true, "keep", true, "auto", "csv").unwrap_err();
        assert!(err.contains("unknown format 'julian'"), "{err}");
    }

    #[test]
    fn custom_without_a_pattern_errors() {
        let err = run(MIXED, "joined", "custom", "", "auto", 68, true, "keep", true, "auto", "csv").unwrap_err();
        assert!(err.contains("custom_format is required"), "{err}");
    }

    #[test]
    fn invalid_custom_pattern_errors() {
        let err = run(MIXED, "joined", "custom", "%Q", "auto", 68, true, "keep", true, "auto", "csv").unwrap_err();
        assert!(err.contains("invalid custom_format"), "{err}");
    }

    #[test]
    fn bad_year_pivot_errors() {
        let err = run(MIXED, "joined", "iso-date", "", "auto", 150, true, "keep", true, "auto", "csv").unwrap_err();
        assert!(err.contains("year_pivot must be between 0 and 99"), "{err}");
    }

    #[test]
    fn unknown_date_order_errors() {
        let err = run(MIXED, "joined", "iso-date", "", "sideways", 68, true, "keep", true, "auto", "csv").unwrap_err();
        assert!(err.contains("unknown date_order"), "{err}");
    }

    #[test]
    fn bad_delimiter_errors() {
        let err = run(MIXED, "joined", "iso-date", "", "auto", 68, true, "keep", true, "::", "csv").unwrap_err();
        assert!(err.contains("delimiter must be"), "{err}");
    }

    #[test]
    fn oversized_input_errors() {
        let big = format!("d\n{}", "2021-06-01\n".repeat(MAX_BYTES / 8));
        let err = run(&big, "d", "iso-date", "", "auto", 68, true, "keep", true, "auto", "csv").unwrap_err();
        assert!(err.contains("the limit is"), "{err}");
    }

    #[test]
    fn header_only_input_errors() {
        let err = run("d\n", "d", "iso-date", "", "auto", 68, true, "keep", true, "auto", "csv").unwrap_err();
        assert!(err.contains("no data rows"), "{err}");
    }
}
