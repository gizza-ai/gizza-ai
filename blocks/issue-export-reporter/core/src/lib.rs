//! issue-export-reporter core — pure compute, shared by the chat skill block and
//! the web page. No wafer/wasm-bindgen deps, no clock, no I/O — fully deterministic.
//!
//! Reads a Jira or Linear CSV issue export and turns it into flow metrics:
//! a status breakdown, lead/cycle-time distributions (median-first percentiles),
//! per-sprint or per-period velocity, and an arrivals-vs-departures burndown.
//!
//! Only the timestamps that are actually present in the export are used — a plain
//! CSV has no status-transition history, so "time in status" is out of reach and
//! the cycle-time clock starts at whichever start column the export carries
//! (`Started`, `In Progress`, …) or at creation when it carries none.

use serde_json::{Map, Value};
use std::collections::HashMap;
use std::fmt::Write as _;

/// Hard cap on the pasted export, mirrored in the descriptor + page copy.
pub const MAX_INPUT_BYTES: usize = 5_000_000;
/// Hard cap on data rows, mirrored in the descriptor + page copy.
pub const MAX_ROWS: usize = 50_000;
/// Hard cap on how many percentiles may be requested at once.
pub const MAX_PERCENTILES: usize = 6;
/// Statuses treated as finished when the caller passes an empty list, mirrored
/// in the descriptor default + page copy.
pub const DEFAULT_DONE_STATUSES: &str = "done,closed,resolved,completed,shipped,released,merged";
/// Statuses treated as dropped work when the caller passes an empty list.
pub const DEFAULT_CANCELLED_STATUSES: &str =
    "cancelled,canceled,won't do,wont do,duplicate,rejected,invalid";
/// Percentiles reported when the caller passes an empty list.
pub const DEFAULT_PERCENTILES: &str = "50,85,95";

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Report {
    Summary,
    Status,
    CycleTime,
    Velocity,
    Burndown,
    Items,
}

impl Report {
    fn parse(s: &str) -> Result<Self, String> {
        match norm_opt(s).as_str() {
            "" | "summary" => Ok(Report::Summary),
            "status" => Ok(Report::Status),
            "cycletime" | "cycle_time" => Ok(Report::CycleTime),
            "velocity" => Ok(Report::Velocity),
            "burndown" => Ok(Report::Burndown),
            "items" => Ok(Report::Items),
            other => Err(format!(
                "unknown report '{other}': expected summary, status, cycle_time, velocity, burndown or items"
            )),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OutFormat {
    Text,
    Json,
    Csv,
    Markdown,
}

impl OutFormat {
    fn parse(s: &str) -> Result<Self, String> {
        match norm_opt(s).as_str() {
            "" | "text" => Ok(OutFormat::Text),
            "json" => Ok(OutFormat::Json),
            "csv" => Ok(OutFormat::Csv),
            "markdown" | "md" => Ok(OutFormat::Markdown),
            other => Err(format!(
                "unknown format '{other}': expected text, json, csv or markdown"
            )),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GroupBy {
    None,
    Assignee,
    Type,
    Priority,
    Sprint,
    Status,
}

impl GroupBy {
    fn parse(s: &str) -> Result<Self, String> {
        match norm_opt(s).as_str() {
            "" | "none" => Ok(GroupBy::None),
            "assignee" => Ok(GroupBy::Assignee),
            "type" => Ok(GroupBy::Type),
            "priority" => Ok(GroupBy::Priority),
            "sprint" => Ok(GroupBy::Sprint),
            "status" => Ok(GroupBy::Status),
            other => Err(format!(
                "unknown group_by '{other}': expected none, assignee, type, priority, sprint or status"
            )),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Period {
    Auto,
    Sprint,
    Week,
    Month,
    Day,
}

impl Period {
    fn parse(s: &str) -> Result<Self, String> {
        match norm_opt(s).as_str() {
            "" | "auto" => Ok(Period::Auto),
            "sprint" | "cycle" => Ok(Period::Sprint),
            "week" => Ok(Period::Week),
            "month" => Ok(Period::Month),
            "day" => Ok(Period::Day),
            other => Err(format!(
                "unknown period '{other}': expected auto, sprint, week, month or day"
            )),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Unit {
    Days,
    Hours,
}

impl Unit {
    fn parse(s: &str) -> Result<Self, String> {
        match norm_opt(s).as_str() {
            "" | "days" | "day" => Ok(Unit::Days),
            "hours" | "hour" => Ok(Unit::Hours),
            other => Err(format!("unknown unit '{other}': expected days or hours")),
        }
    }
    fn secs(self) -> f64 {
        match self {
            Unit::Days => 86_400.0,
            Unit::Hours => 3_600.0,
        }
    }
    fn label(self) -> &'static str {
        match self {
            Unit::Days => "days",
            Unit::Hours => "hours",
        }
    }
}

/// Lowercase + trim an option value, folding `-` and spaces onto `_`-free form.
fn norm_opt(s: &str) -> String {
    s.trim()
        .to_ascii_lowercase()
        .chars()
        .filter(|c| !matches!(c, ' ' | '-'))
        .collect()
}

/// Normalize a header name to letters+digits only, lowercased.
fn norm_header(s: &str) -> String {
    s.trim()
        .to_ascii_lowercase()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect()
}

fn delim_byte(d: &str, first_line: &str) -> Result<u8, String> {
    match norm_opt(d).as_str() {
        "" | "auto" => {
            let counts = [
                (b',', first_line.matches(',').count()),
                (b';', first_line.matches(';').count()),
                (b'\t', first_line.matches('\t').count()),
                (b'|', first_line.matches('|').count()),
            ];
            let best = counts.iter().max_by_key(|(_, n)| *n).unwrap();
            Ok(if best.1 == 0 { b',' } else { best.0 })
        }
        "comma" => Ok(b','),
        "semicolon" => Ok(b';'),
        "tab" => Ok(b'\t'),
        "pipe" => Ok(b'|'),
        other => Err(format!(
            "unknown delimiter '{other}': expected auto, comma, semicolon, tab or pipe"
        )),
    }
}

// ---------------------------------------------------------------------------
// Calendar helpers (no clock — every timestamp comes from the export)
// ---------------------------------------------------------------------------

const MONTH_NAMES: [&str; 12] = [
    "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
];

/// Days since 1970-01-01 for a proleptic-Gregorian civil date (Howard Hinnant's
/// `days_from_civil`).
fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = if m > 2 { m - 3 } else { m + 9 } as i64;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Inverse of [`days_from_civil`].
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// 0 = Sunday … 6 = Saturday (1970-01-01 was a Thursday).
fn weekday(day: i64) -> i64 {
    (day + 4).rem_euclid(7)
}

fn is_weekend(day: i64) -> bool {
    matches!(weekday(day), 0 | 6)
}

/// Weekdays in the half-open day range `[from, to)`.
fn weekdays_between(from: i64, to: i64) -> i64 {
    if to <= from {
        return 0;
    }
    let n = to - from;
    let full = n / 7;
    let rem = n % 7;
    let mut count = full * 5;
    for i in 0..rem {
        if !is_weekend(from + full * 7 + i) {
            count += 1;
        }
    }
    count
}

/// Elapsed seconds between two epoch timestamps, optionally counting only the
/// Monday–Friday portion. Negative spans clamp to zero (the caller counts them).
fn elapsed_secs(start: i64, end: i64, business: bool) -> f64 {
    if end <= start {
        return 0.0;
    }
    if !business {
        return (end - start) as f64;
    }
    let (da, db) = (start.div_euclid(86_400), end.div_euclid(86_400));
    if da == db {
        return if is_weekend(da) {
            0.0
        } else {
            (end - start) as f64
        };
    }
    let mut total: i64 = 0;
    if !is_weekend(da) {
        total += (da + 1) * 86_400 - start;
    }
    if !is_weekend(db) {
        total += end - db * 86_400;
    }
    total += weekdays_between(da + 1, db) * 86_400;
    total as f64
}

fn month_index(name: &str) -> Option<u32> {
    let n = name.trim().to_ascii_lowercase();
    if n.len() < 3 || !n.chars().all(|c| c.is_ascii_alphabetic()) {
        return None;
    }
    MONTH_NAMES
        .iter()
        .position(|m| n.starts_with(m))
        .map(|i| i as u32 + 1)
}

fn parse_hms(t: &str) -> Option<(i64, i64, i64)> {
    let mut it = t.split(':');
    let h: i64 = it.next()?.parse().ok()?;
    let m: i64 = it.next().unwrap_or("0").parse().ok()?;
    let rest = it.next().unwrap_or("0");
    let s: i64 = rest.split('.').next()?.parse().ok()?;
    if !(0..=24).contains(&h) || !(0..=59).contains(&m) || !(0..=60).contains(&s) {
        return None;
    }
    Some((h, m, s))
}

/// ISO-ish: `YYYY-MM-DD`, optionally `T`/space + `HH:MM[:SS[.fff]]`, optionally
/// `Z`, `±HH:MM` or `±HHMM`.
fn parse_iso(t: &str) -> Option<i64> {
    let b = t.as_bytes();
    if b.len() < 10 {
        return None;
    }
    let sep = b[4] as char;
    if !(sep == '-' || sep == '/') || b[7] as char != sep {
        return None;
    }
    let y: i64 = t[0..4].parse().ok()?;
    let mo: u32 = t[5..7].parse().ok()?;
    let d: u32 = t[8..10].parse().ok()?;
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) {
        return None;
    }
    let mut secs = days_from_civil(y, mo, d) * 86_400;
    let rest = t[10..].trim();
    if rest.is_empty() {
        return Some(secs);
    }
    let rest = rest
        .strip_prefix('T')
        .or_else(|| rest.strip_prefix('t'))
        .unwrap_or(rest)
        .trim();
    // Split the offset off the tail.
    let (time_part, offset) = split_offset(rest)?;
    let (h, mi, s) = parse_hms(time_part)?;
    secs += h * 3600 + mi * 60 + s;
    Some(secs - offset)
}

/// Returns `(time_text, offset_seconds)`; `Z`/absent means UTC.
fn split_offset(rest: &str) -> Option<(&str, i64)> {
    let r = rest.trim();
    if let Some(stripped) = r.strip_suffix('Z').or_else(|| r.strip_suffix('z')) {
        return Some((stripped.trim(), 0));
    }
    // Look for a +/- after the time (skip the first char so a leading sign can't match).
    if let Some(pos) = r.rfind(['+', '-']) {
        if pos > 0 {
            let (time_part, off) = r.split_at(pos);
            let sign = if off.starts_with('-') { -1 } else { 1 };
            let digits: String = off[1..].chars().filter(|c| c.is_ascii_digit()).collect();
            let (oh, om) = match digits.len() {
                2 => (digits.parse::<i64>().ok()?, 0),
                4 => (
                    digits[0..2].parse::<i64>().ok()?,
                    digits[2..4].parse::<i64>().ok()?,
                ),
                _ => return None,
            };
            return Some((time_part.trim(), sign * (oh * 3600 + om * 60)));
        }
    }
    Some((r, 0))
}

/// Jira's classic export stamp: `05/Jan/24 9:07 AM`, `5-Jan-2024 09:07:33`.
fn parse_jira(t: &str) -> Option<i64> {
    let mut it = t.split_whitespace();
    let date_part = it.next()?;
    let parts: Vec<&str> = date_part.split(['/', '-', '.']).collect();
    if parts.len() != 3 {
        return None;
    }
    let d: u32 = parts[0].trim().parse().ok()?;
    let mo = month_index(parts[1])?;
    let ytxt = parts[2].trim();
    let mut y: i64 = ytxt.parse().ok()?;
    if ytxt.len() <= 2 {
        y += if y < 70 { 2000 } else { 1900 };
    }
    if !(1..=31).contains(&d) {
        return None;
    }
    let (mut h, mut mi, mut s) = (0i64, 0i64, 0i64);
    if let Some(tp) = it.next() {
        let (a, b, c) = parse_hms(tp)?;
        h = a;
        mi = b;
        s = c;
    }
    if let Some(mer) = it.next() {
        match mer.trim().to_ascii_uppercase().as_str() {
            "AM" => {
                if h == 12 {
                    h = 0
                }
            }
            "PM" => {
                if h < 12 {
                    h += 12
                }
            }
            _ => return None,
        }
    }
    Some(days_from_civil(y, mo, d) * 86_400 + h * 3600 + mi * 60 + s)
}

/// `None` for a blank cell; `Err` for a non-blank cell we cannot read.
fn parse_ts(raw: &str) -> Result<Option<i64>, ()> {
    let t = raw.trim();
    if t.is_empty() || t == "-" {
        return Ok(None);
    }
    parse_iso(t).or_else(|| parse_jira(t)).map(Some).ok_or(())
}

fn iso_date(day: i64) -> String {
    let (y, m, d) = civil_from_days(day);
    format!("{y:04}-{m:02}-{d:02}")
}

// ---------------------------------------------------------------------------
// Column mapping
// ---------------------------------------------------------------------------

#[derive(Default)]
struct Cols {
    key: Option<usize>,
    summary: Option<usize>,
    status: Option<usize>,
    created: Option<usize>,
    started: Option<usize>,
    resolved: Option<usize>,
    points: Option<usize>,
    sprint: Vec<usize>,
    assignee: Option<usize>,
    itype: Option<usize>,
    priority: Option<usize>,
}

const KEY_NAMES: [&str; 6] = ["issuekey", "key", "identifier", "id", "issueid", "ticket"];
const SUMMARY_NAMES: [&str; 3] = ["summary", "title", "name"];
const STATUS_NAMES: [&str; 4] = ["status", "state", "statusname", "workflowstatus"];
const CREATED_NAMES: [&str; 5] = [
    "created",
    "createdat",
    "createddate",
    "createdtime",
    "creationdate",
];
const STARTED_NAMES: [&str; 7] = [
    "started",
    "startedat",
    "startdate",
    "startedon",
    "inprogress",
    "inprogressat",
    "activatedat",
];
const RESOLVED_NAMES: [&str; 11] = [
    "resolved",
    "resolvedat",
    "resolutiondate",
    "resolveddate",
    "completed",
    "completedat",
    "completeddate",
    "closed",
    "closedat",
    "doneat",
    "donedate",
];
const POINTS_NAMES: [&str; 6] = [
    "storypoints",
    "storypointestimate",
    "points",
    "estimate",
    "estimatepoints",
    "effort",
];
const SPRINT_NAMES: [&str; 6] = [
    "sprint",
    "sprintname",
    "cycle",
    "cyclename",
    "iteration",
    "iterationname",
];
const ASSIGNEE_NAMES: [&str; 5] = [
    "assignee",
    "assigneename",
    "assignedto",
    "owner",
    "assigneedisplayname",
];
const TYPE_NAMES: [&str; 4] = ["issuetype", "type", "worktype", "itemtype"];
const PRIORITY_NAMES: [&str; 2] = ["priority", "prioritylabel"];

fn find_first(headers: &[String], candidates: &[&str]) -> Option<usize> {
    for cand in candidates {
        if let Some(i) = headers.iter().position(|h| h == cand) {
            return Some(i);
        }
    }
    None
}

fn find_all(headers: &[String], candidates: &[&str]) -> Vec<usize> {
    headers
        .iter()
        .enumerate()
        .filter(|(_, h)| candidates.contains(&h.as_str()))
        .map(|(i, _)| i)
        .collect()
}

const FIELD_NAMES: [&str; 11] = [
    "key", "summary", "status", "created", "started", "resolved", "points", "sprint", "assignee",
    "type", "priority",
];

fn apply_overrides(
    cols: &mut Cols,
    spec: &str,
    normed: &[String],
    raw_headers: &[String],
) -> Result<(), String> {
    for pair in spec.split(',') {
        let p = pair.trim();
        if p.is_empty() {
            continue;
        }
        let (field, header) = p.split_once('=').ok_or_else(|| {
            format!(
                "column mapping '{p}' is not in field=header form — write e.g. columns='status=State, resolved=Done At'"
            )
        })?;
        let field = norm_opt(field);
        let header = header.trim();
        let idx = match normed.iter().position(|h| *h == norm_header(header)) {
            Some(i) => i,
            None => match header.parse::<usize>() {
                Ok(n) if n >= 1 && n <= normed.len() => n - 1,
                _ => {
                    return Err(format!(
                        "no column named '{header}' in the export (available: {}); you can also give a 1-based column number",
                        preview_headers(raw_headers)
                    ))
                }
            },
        };
        match field.as_str() {
            "key" => cols.key = Some(idx),
            "summary" | "title" => cols.summary = Some(idx),
            "status" => cols.status = Some(idx),
            "created" => cols.created = Some(idx),
            "started" => cols.started = Some(idx),
            "resolved" | "completed" => cols.resolved = Some(idx),
            "points" | "estimate" => cols.points = Some(idx),
            "sprint" | "cycle" => cols.sprint = vec![idx],
            "assignee" => cols.assignee = Some(idx),
            "type" => cols.itype = Some(idx),
            "priority" => cols.priority = Some(idx),
            other => {
                return Err(format!(
                    "unknown mapping field '{other}': expected one of {}",
                    FIELD_NAMES.join(", ")
                ))
            }
        }
    }
    Ok(())
}

fn preview_headers(headers: &[String]) -> String {
    let shown: Vec<String> = headers.iter().take(15).map(|h| h.to_string()).collect();
    let mut s = shown.join(", ");
    if headers.len() > 15 {
        let _ = write!(s, ", … ({} columns)", headers.len());
    }
    s
}

// ---------------------------------------------------------------------------
// Items
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum State {
    Completed,
    Cancelled,
    Open,
}

struct Item {
    key: String,
    status: String,
    assignee: String,
    itype: String,
    priority: String,
    sprint: String,
    points: Option<f64>,
    created: Option<i64>,
    started: Option<i64>,
    resolved: Option<i64>,
    state: State,
}

fn cell(rec: &csv::StringRecord, idx: Option<usize>) -> String {
    idx.and_then(|i| rec.get(i))
        .unwrap_or("")
        .trim()
        .to_string()
}

/// Jira repeats the `Sprint` header once per sprint an issue has been in; the
/// last non-empty one is the sprint it finished in.
fn last_non_empty(rec: &csv::StringRecord, idxs: &[usize]) -> String {
    let mut out = String::new();
    for &i in idxs {
        if let Some(v) = rec.get(i) {
            if !v.trim().is_empty() {
                out = v.trim().to_string();
            }
        }
    }
    out
}

fn split_list(s: &str) -> Vec<String> {
    s.split(',')
        .map(|p| p.trim().to_ascii_lowercase())
        .filter(|p| !p.is_empty())
        .collect()
}

// ---------------------------------------------------------------------------
// Output model
// ---------------------------------------------------------------------------

#[derive(Clone)]
enum Cell {
    S(String),
    N(f64),
    I(i64),
    Pct(f64),
    Null,
}

impl Cell {
    fn text(&self) -> String {
        match self {
            Cell::S(s) => s.clone(),
            Cell::N(n) => fmt_num(*n),
            Cell::I(i) => i.to_string(),
            Cell::Pct(p) => format!("{}%", fmt_num(*p)),
            Cell::Null => "—".into(),
        }
    }
    fn plain(&self) -> String {
        match self {
            Cell::Null => String::new(),
            Cell::Pct(p) => fmt_num(*p),
            other => other.text(),
        }
    }
    fn json(&self) -> Value {
        match self {
            Cell::S(s) => Value::String(s.clone()),
            Cell::N(n) | Cell::Pct(n) => serde_json::Number::from_f64(*n)
                .map(Value::Number)
                .unwrap_or(Value::Null),
            Cell::I(i) => Value::Number((*i).into()),
            Cell::Null => Value::Null,
        }
    }
    fn numeric(&self) -> bool {
        matches!(self, Cell::N(_) | Cell::I(_) | Cell::Pct(_))
    }
}

fn fmt_num(n: f64) -> String {
    if !n.is_finite() {
        return "—".into();
    }
    let r = (n * 100.0).round() / 100.0;
    if (r - r.round()).abs() < f64::EPSILON {
        format!("{}", r.round() as i64)
    } else {
        let s = format!("{r:.2}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

struct Table {
    key: String,
    title: String,
    columns: Vec<(String, String)>, // (json key, display label)
    rows: Vec<Vec<Cell>>,
}

struct ReportOut {
    report: &'static str,
    source: &'static str,
    columns_used: Vec<(String, String)>,
    tables: Vec<Table>,
    notes: Vec<String>,
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Parse a Jira/Linear CSV export and render the requested report.
#[allow(clippy::too_many_arguments)]
pub fn run(
    data: &str,
    report: &str,
    format: &str,
    delimiter: &str,
    columns: &str,
    done_statuses: &str,
    cancelled_statuses: &str,
    group_by: &str,
    period: &str,
    unit: &str,
    business_days: bool,
    percentiles: &str,
) -> Result<String, String> {
    if data.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "export is {} bytes, over the {MAX_INPUT_BYTES} byte limit — export fewer issues or fewer columns",
            data.len()
        ));
    }
    let trimmed = data.trim_start_matches('\u{feff}');
    if trimmed.trim().is_empty() {
        return Err("no CSV data: paste a Jira or Linear issue export, header row first".into());
    }

    let report = Report::parse(report)?;
    let out_format = OutFormat::parse(format)?;
    let group_by = GroupBy::parse(group_by)?;
    let mut period = Period::parse(period)?;
    let unit = Unit::parse(unit)?;
    let pcts = parse_percentiles(percentiles)?;
    let first_line = trimmed.lines().next().unwrap_or("");
    let delim = delim_byte(delimiter, first_line)?;

    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(delim)
        .flexible(true)
        .from_reader(trimmed.as_bytes());
    let raw_headers: Vec<String> = rdr
        .headers()
        .map_err(|e| format!("could not read the header row: {e}"))?
        .iter()
        .map(|h| h.trim().to_string())
        .collect();
    if raw_headers.is_empty() {
        return Err("the export has no header row".into());
    }
    let normed: Vec<String> = raw_headers.iter().map(|h| norm_header(h)).collect();

    let mut cols = Cols {
        key: find_first(&normed, &KEY_NAMES),
        summary: find_first(&normed, &SUMMARY_NAMES),
        status: find_first(&normed, &STATUS_NAMES),
        created: find_first(&normed, &CREATED_NAMES),
        started: find_first(&normed, &STARTED_NAMES),
        resolved: find_first(&normed, &RESOLVED_NAMES),
        points: find_first(&normed, &POINTS_NAMES),
        sprint: find_all(&normed, &SPRINT_NAMES),
        assignee: find_first(&normed, &ASSIGNEE_NAMES),
        itype: find_first(&normed, &TYPE_NAMES),
        priority: find_first(&normed, &PRIORITY_NAMES),
    };
    if !columns.trim().is_empty() {
        apply_overrides(&mut cols, columns, &normed, &raw_headers)?;
    }
    // A "started" column that resolved to the same column as created/resolved is
    // a mis-detection, not a cycle-time clock.
    if cols.started.is_some() && (cols.started == cols.created || cols.started == cols.resolved) {
        cols.started = None;
    }
    if cols.status.is_none() && cols.resolved.is_none() {
        return Err(format!(
            "no Status column and no completion-date column found (looked for Status/State and Resolved/Completed/Closed). Columns in this export: {}. Map one with columns='status=<header>'.",
            preview_headers(&raw_headers)
        ));
    }

    let source = if normed.iter().any(|h| {
        h == "issuekey" || h == "issuetype" || h == "issueid" || h == "resolutiondate"
    }) {
        "jira"
    } else if normed
        .iter()
        .any(|h| h == "cyclename" || h == "identifier" || h == "cycle")
    {
        "linear"
    } else {
        "generic"
    };

    // An omitted / blank list means "use the built-in list", so every surface
    // (chat, CLI, page) agrees without each one restating the defaults.
    let done_list = split_list(if done_statuses.trim().is_empty() {
        DEFAULT_DONE_STATUSES
    } else {
        done_statuses
    });
    let cancelled_list = split_list(if cancelled_statuses.trim().is_empty() {
        DEFAULT_CANCELLED_STATUSES
    } else {
        cancelled_statuses
    });

    let mut items: Vec<Item> = Vec::new();
    let mut negative_spans = 0usize;
    for (i, rec) in rdr.records().enumerate() {
        let rec = rec.map_err(|e| format!("row {}: could not read the CSV: {e}", i + 2))?;
        if rec.iter().all(|f| f.trim().is_empty()) {
            continue;
        }
        if items.len() >= MAX_ROWS {
            return Err(format!(
                "export has more than {MAX_ROWS} issue rows — filter the export or split it before pasting"
            ));
        }
        let row_no = i + 2;
        let status = cell(&rec, cols.status);
        let status_l = status.to_ascii_lowercase();
        let created = read_ts(&rec, cols.created, row_no, "created")?;
        let started = read_ts(&rec, cols.started, row_no, "started")?;
        let resolved = read_ts(&rec, cols.resolved, row_no, "resolved")?;
        let state = if cancelled_list.contains(&status_l) {
            State::Cancelled
        } else if done_list.contains(&status_l) || resolved.is_some() {
            State::Completed
        } else {
            State::Open
        };
        if let (Some(c), Some(r)) = (created, resolved) {
            if r < c {
                negative_spans += 1;
            }
        }
        let points = cols
            .points
            .and_then(|i| rec.get(i))
            .map(|v| v.trim().replace(',', ""))
            .and_then(|v| v.parse::<f64>().ok())
            .filter(|p| p.is_finite());
        items.push(Item {
            key: cell(&rec, cols.key),
            status,
            assignee: cell(&rec, cols.assignee),
            itype: cell(&rec, cols.itype),
            priority: cell(&rec, cols.priority),
            sprint: last_non_empty(&rec, &cols.sprint),
            points,
            created,
            started,
            resolved,
            state,
        });
    }
    if items.is_empty() {
        return Err("the export has a header row but no issue rows".into());
    }

    let has_sprint = items.iter().any(|i| !i.sprint.is_empty());
    if period == Period::Sprint && !has_sprint {
        return Err(
            "no Sprint or Cycle column with values was found — use period=week, month or day, or map one with columns='sprint=<header>'"
                .into(),
        );
    }
    if period == Period::Auto {
        period = if has_sprint && report != Report::Burndown {
            Period::Sprint
        } else {
            Period::Week
        };
    }
    if report == Report::Burndown && period == Period::Sprint {
        return Err(
            "a burndown needs calendar buckets — use period=week, month or day (sprint names have no calendar order)"
                .into(),
        );
    }

    let mut notes: Vec<String> = Vec::new();
    if negative_spans > 0 {
        notes.push(format!(
            "{negative_spans} issue(s) are resolved before they were created; those spans were counted as 0"
        ));
    }
    if business_days {
        notes.push("Elapsed times exclude Saturdays and Sundays.".into());
    }

    let mut columns_used: Vec<(String, String)> = Vec::new();
    let mut push_col = |name: &str, idx: Option<usize>| {
        if let Some(i) = idx {
            columns_used.push((name.to_string(), raw_headers[i].clone()));
        }
    };
    push_col("key", cols.key);
    push_col("status", cols.status);
    push_col("created", cols.created);
    push_col("started", cols.started);
    push_col("resolved", cols.resolved);
    push_col("points", cols.points);
    push_col("assignee", cols.assignee);
    push_col("type", cols.itype);
    push_col("priority", cols.priority);
    if let Some(&i) = cols.sprint.first() {
        columns_used.push(("sprint".into(), raw_headers[i].clone()));
    }

    let ctx = Ctx {
        unit,
        business_days,
        pcts: &pcts,
        group_by,
        period,
        has_points: cols.points.is_some(),
        has_started: cols.started.is_some(),
    };

    let mut tables: Vec<Table> = Vec::new();
    match report {
        Report::Summary => {
            tables.push(overview_table(&items, &ctx));
            tables.push(status_table(&items, &ctx));
            tables.extend(duration_tables(&items, &ctx, &mut notes));
            tables.push(velocity_table(&items, &ctx));
        }
        Report::Status => tables.push(status_table(&items, &ctx)),
        Report::CycleTime => tables.extend(duration_tables(&items, &ctx, &mut notes)),
        Report::Velocity => {
            tables.push(velocity_table(&items, &ctx));
            tables.push(velocity_summary_table(&items, &ctx));
        }
        Report::Burndown => tables.push(burndown_table(&items, &ctx, &mut notes)),
        Report::Items => tables.push(items_table(&items, &ctx)),
    }

    let out = ReportOut {
        report: match report {
            Report::Summary => "summary",
            Report::Status => "status",
            Report::CycleTime => "cycle_time",
            Report::Velocity => "velocity",
            Report::Burndown => "burndown",
            Report::Items => "items",
        },
        source,
        columns_used,
        tables,
        notes,
    };

    Ok(match out_format {
        OutFormat::Text => render_text(&out),
        OutFormat::Markdown => render_markdown(&out),
        OutFormat::Csv => render_csv(&out),
        OutFormat::Json => render_json(&out),
    })
}

fn read_ts(
    rec: &csv::StringRecord,
    idx: Option<usize>,
    row: usize,
    field: &str,
) -> Result<Option<i64>, String> {
    let raw = match idx.and_then(|i| rec.get(i)) {
        Some(v) => v,
        None => return Ok(None),
    };
    parse_ts(raw).map_err(|_| {
        format!(
            "row {row}: could not read the {field} timestamp '{}' — expected 2024-01-05, 2024-01-05T09:07:33Z, 2024-01-05 09:07 or a Jira-style 05/Jan/24 9:07 AM",
            raw.trim()
        )
    })
}

fn parse_percentiles(spec: &str) -> Result<Vec<f64>, String> {
    let s = spec.trim();
    if s.is_empty() {
        return Ok(vec![50.0, 85.0, 95.0]);
    }
    let mut out = Vec::new();
    for part in s.split(',') {
        let p = part.trim().trim_start_matches(['p', 'P']);
        if p.is_empty() {
            continue;
        }
        let v: f64 = p
            .parse()
            .map_err(|_| format!("percentile '{}' is not a number — write e.g. 50,85,95", part.trim()))?;
        if !(1.0..=99.0).contains(&v) {
            return Err(format!(
                "percentile {} is out of range — use values between 1 and 99 (the maximum is reported separately)",
                fmt_num(v)
            ));
        }
        out.push(v);
    }
    if out.is_empty() {
        return Err("no percentiles given — write e.g. 50,85,95".into());
    }
    if out.len() > MAX_PERCENTILES {
        return Err(format!(
            "{} percentiles requested, at most {MAX_PERCENTILES} are reported at once",
            out.len()
        ));
    }
    Ok(out)
}

struct Ctx<'a> {
    unit: Unit,
    business_days: bool,
    pcts: &'a [f64],
    group_by: GroupBy,
    period: Period,
    has_points: bool,
    has_started: bool,
}

/// Nearest-rank percentile: the smallest observed value at or below which at
/// least `p`% of the sorted sample falls.
fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return f64::NAN;
    }
    let rank = (p / 100.0 * sorted.len() as f64).ceil() as usize;
    let idx = rank.saturating_sub(1).min(sorted.len() - 1);
    sorted[idx]
}

// ---------------------------------------------------------------------------
// Tables
// ---------------------------------------------------------------------------

fn overview_table(items: &[Item], ctx: &Ctx) -> Table {
    let total = items.len();
    let completed = items.iter().filter(|i| i.state == State::Completed).count();
    let cancelled = items.iter().filter(|i| i.state == State::Cancelled).count();
    let open = total - completed - cancelled;
    let mut rows = vec![
        vec![Cell::S("Issues".into()), Cell::I(total as i64)],
        vec![Cell::S("Completed".into()), Cell::I(completed as i64)],
        vec![Cell::S("Open".into()), Cell::I(open as i64)],
        vec![Cell::S("Cancelled".into()), Cell::I(cancelled as i64)],
        vec![
            Cell::S("Completion rate".into()),
            Cell::Pct(pct(completed, total)),
        ],
    ];
    if ctx.has_points {
        let all: f64 = items.iter().filter_map(|i| i.points).sum();
        let done: f64 = items
            .iter()
            .filter(|i| i.state == State::Completed)
            .filter_map(|i| i.points)
            .sum();
        rows.push(vec![Cell::S("Points total".into()), Cell::N(all)]);
        rows.push(vec![Cell::S("Points completed".into()), Cell::N(done)]);
    }
    let created: Vec<i64> = items.iter().filter_map(|i| i.created).collect();
    if let (Some(min), Some(max)) = (created.iter().min(), created.iter().max()) {
        rows.push(vec![
            Cell::S("Created range".into()),
            Cell::S(format!(
                "{} → {}",
                iso_date(min.div_euclid(86_400)),
                iso_date(max.div_euclid(86_400))
            )),
        ]);
    }
    Table {
        key: "overview".into(),
        title: "Overview".into(),
        columns: vec![
            ("metric".into(), "Metric".into()),
            ("value".into(), "Value".into()),
        ],
        rows,
    }
}

fn pct(part: usize, total: usize) -> f64 {
    if total == 0 {
        0.0
    } else {
        (part as f64 * 1000.0 / total as f64).round() / 10.0
    }
}

fn status_table(items: &[Item], ctx: &Ctx) -> Table {
    let mut counts: HashMap<String, (usize, f64)> = HashMap::new();
    for it in items {
        let name = if it.status.is_empty() {
            "(no status)".to_string()
        } else {
            it.status.clone()
        };
        let e = counts.entry(name).or_insert((0, 0.0));
        e.0 += 1;
        e.1 += it.points.unwrap_or(0.0);
    }
    let mut rows: Vec<(String, usize, f64)> =
        counts.into_iter().map(|(k, v)| (k, v.0, v.1)).collect();
    rows.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    let mut columns = vec![
        ("status".into(), "Status".into()),
        ("issues".into(), "Issues".into()),
        ("share".into(), "Share".into()),
    ];
    if ctx.has_points {
        columns.push(("points".to_string(), "Points".to_string()));
    }
    let total = items.len();
    let rows = rows
        .into_iter()
        .map(|(name, n, pts)| {
            let mut r = vec![Cell::S(name), Cell::I(n as i64), Cell::Pct(pct(n, total))];
            if ctx.has_points {
                r.push(Cell::N(pts));
            }
            r
        })
        .collect();
    Table {
        key: "status_breakdown".into(),
        title: "Status breakdown".into(),
        columns,
        rows,
    }
}

fn group_label(it: &Item, by: GroupBy) -> String {
    let raw = match by {
        GroupBy::None => return "All issues".into(),
        GroupBy::Assignee => &it.assignee,
        GroupBy::Type => &it.itype,
        GroupBy::Priority => &it.priority,
        GroupBy::Sprint => &it.sprint,
        GroupBy::Status => &it.status,
    };
    if raw.is_empty() {
        match by {
            GroupBy::Assignee => "(unassigned)".into(),
            GroupBy::Sprint => "(no sprint)".into(),
            _ => "(none)".into(),
        }
    } else {
        raw.clone()
    }
}

/// Lead-time (created → resolved) and, when the export carries a start column,
/// cycle-time (started → resolved) distributions.
fn duration_tables(items: &[Item], ctx: &Ctx, notes: &mut Vec<String>) -> Vec<Table> {
    let mut out = Vec::new();
    let unit_note = if ctx.business_days {
        format!("business {}", ctx.unit.label())
    } else {
        format!("calendar {}", ctx.unit.label())
    };
    out.push(distribution_table(
        items,
        ctx,
        "lead_time",
        &format!("Lead time — created to resolved, in {unit_note}"),
        |i| match (i.created, i.resolved) {
            (Some(a), Some(b)) if i.state == State::Completed => Some((a, b)),
            _ => None,
        },
    ));
    if ctx.has_started {
        out.push(distribution_table(
            items,
            ctx,
            "cycle_time",
            &format!("Cycle time — started to resolved, in {unit_note}"),
            |i| match (i.started, i.resolved) {
                (Some(a), Some(b)) if i.state == State::Completed => Some((a, b)),
                _ => None,
            },
        ));
    } else {
        notes.push(
            "No start column (Started / In Progress) was found, so only lead time is reported. Map one with columns='started=<header>' to get cycle time too."
                .into(),
        );
    }
    out
}

fn distribution_table<F>(items: &[Item], ctx: &Ctx, key: &str, title: &str, span: F) -> Table
where
    F: Fn(&Item) -> Option<(i64, i64)>,
{
    let mut groups: Vec<(String, Vec<f64>)> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();
    for it in items {
        let Some((a, b)) = span(it) else { continue };
        let v = elapsed_secs(a, b, ctx.business_days) / ctx.unit.secs();
        let label = group_label(it, ctx.group_by);
        match index.get(&label) {
            Some(&i) => groups[i].1.push(v),
            None => {
                index.insert(label.clone(), groups.len());
                groups.push((label, vec![v]));
            }
        }
    }
    for g in groups.iter_mut() {
        g.1.sort_by(|a, b| a.partial_cmp(b).unwrap());
    }
    groups.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then_with(|| a.0.cmp(&b.0)));

    let mut columns = vec![
        (
            "group".to_string(),
            match ctx.group_by {
                GroupBy::None => "Scope".to_string(),
                GroupBy::Assignee => "Assignee".to_string(),
                GroupBy::Type => "Type".to_string(),
                GroupBy::Priority => "Priority".to_string(),
                GroupBy::Sprint => "Sprint".to_string(),
                GroupBy::Status => "Status".to_string(),
            },
        ),
        ("issues".into(), "Issues".into()),
        ("min".into(), "Min".into()),
    ];
    for p in ctx.pcts {
        columns.push((format!("p{}", fmt_num(*p)), format!("p{}", fmt_num(*p))));
    }
    columns.push(("max".into(), "Max".into()));
    columns.push(("mean".into(), "Mean".into()));

    let rows = groups
        .into_iter()
        .map(|(label, vals)| {
            let mut r = vec![
                Cell::S(label),
                Cell::I(vals.len() as i64),
                Cell::N(vals[0]),
            ];
            for p in ctx.pcts {
                r.push(Cell::N(percentile(&vals, *p)));
            }
            r.push(Cell::N(vals[vals.len() - 1]));
            r.push(Cell::N(vals.iter().sum::<f64>() / vals.len() as f64));
            r
        })
        .collect::<Vec<_>>();

    Table {
        key: key.to_string(),
        title: if rows.is_empty() {
            format!("{title} — no completed issue has both timestamps")
        } else {
            title.to_string()
        },
        columns,
        rows,
    }
}

fn period_label(ts: i64, period: Period) -> String {
    let day = ts.div_euclid(86_400);
    match period {
        Period::Day => iso_date(day),
        Period::Week => {
            let back = (weekday(day) + 6).rem_euclid(7);
            iso_date(day - back)
        }
        Period::Month => {
            let (y, m, _) = civil_from_days(day);
            format!("{y:04}-{m:02}")
        }
        _ => iso_date(day),
    }
}

/// Completed issues bucketed by sprint or by the calendar period they finished in.
fn velocity_buckets(items: &[Item], ctx: &Ctx) -> (Vec<(String, usize, f64)>, usize) {
    let mut order: Vec<String> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();
    let mut rows: Vec<(String, usize, f64)> = Vec::new();
    let mut unbucketed = 0usize;
    for it in items.iter().filter(|i| i.state == State::Completed) {
        let label = if ctx.period == Period::Sprint {
            if it.sprint.is_empty() {
                "(no sprint)".to_string()
            } else {
                it.sprint.clone()
            }
        } else {
            match it.resolved {
                Some(ts) => period_label(ts, ctx.period),
                None => {
                    unbucketed += 1;
                    continue;
                }
            }
        };
        let i = match index.get(&label) {
            Some(&i) => i,
            None => {
                index.insert(label.clone(), rows.len());
                order.push(label.clone());
                rows.push((label, 0, 0.0));
                rows.len() - 1
            }
        };
        rows[i].1 += 1;
        rows[i].2 += it.points.unwrap_or(0.0);
    }
    if ctx.period != Period::Sprint {
        rows.sort_by(|a, b| a.0.cmp(&b.0));
    }
    let _ = order;
    (rows, unbucketed)
}

fn velocity_title(ctx: &Ctx) -> String {
    match ctx.period {
        Period::Sprint => "Velocity by sprint".into(),
        Period::Week => "Velocity by week (week starting Monday)".into(),
        Period::Month => "Velocity by month".into(),
        Period::Day => "Velocity by day".into(),
        Period::Auto => "Velocity".into(),
    }
}

fn velocity_table(items: &[Item], ctx: &Ctx) -> Table {
    let (buckets, _) = velocity_buckets(items, ctx);
    let mut columns = vec![
        (
            "period".to_string(),
            if ctx.period == Period::Sprint {
                "Sprint".to_string()
            } else {
                "Period".to_string()
            },
        ),
        ("completed".into(), "Completed".into()),
    ];
    if ctx.has_points {
        columns.push(("points".to_string(), "Points".to_string()));
    }
    let rows = buckets
        .into_iter()
        .map(|(label, n, pts)| {
            let mut r = vec![Cell::S(label), Cell::I(n as i64)];
            if ctx.has_points {
                r.push(Cell::N(pts));
            }
            r
        })
        .collect();
    Table {
        key: "velocity".into(),
        title: velocity_title(ctx),
        columns,
        rows,
    }
}

fn velocity_summary_table(items: &[Item], ctx: &Ctx) -> Table {
    let (buckets, unbucketed) = velocity_buckets(items, ctx);
    let periods = buckets.len();
    let total: usize = buckets.iter().map(|b| b.1).sum();
    let points: f64 = buckets.iter().map(|b| b.2).sum();
    let mut rows = vec![
        vec![
            Cell::S(if ctx.period == Period::Sprint {
                "Sprints".into()
            } else {
                "Periods".to_string()
            }),
            Cell::I(periods as i64),
        ],
        vec![Cell::S("Completed issues".into()), Cell::I(total as i64)],
        vec![
            Cell::S("Average issues per period".into()),
            if periods == 0 {
                Cell::Null
            } else {
                Cell::N(total as f64 / periods as f64)
            },
        ],
    ];
    if ctx.has_points {
        rows.push(vec![Cell::S("Completed points".into()), Cell::N(points)]);
        rows.push(vec![
            Cell::S("Average points per period".into()),
            if periods == 0 {
                Cell::Null
            } else {
                Cell::N(points / periods as f64)
            },
        ]);
    }
    if unbucketed > 0 {
        rows.push(vec![
            Cell::S("Completed without a date".into()),
            Cell::I(unbucketed as i64),
        ]);
    }
    Table {
        key: "velocity_summary".into(),
        title: "Velocity summary".into(),
        columns: vec![
            ("metric".into(), "Metric".into()),
            ("value".into(), "Value".into()),
        ],
        rows,
    }
}

fn burndown_table(items: &[Item], ctx: &Ctx, notes: &mut Vec<String>) -> Table {
    let mut created_at: HashMap<String, usize> = HashMap::new();
    let mut done_at: HashMap<String, usize> = HashMap::new();
    let mut skipped = 0usize;
    for it in items {
        match it.created {
            Some(ts) => *created_at.entry(period_label(ts, ctx.period)).or_insert(0) += 1,
            None => skipped += 1,
        }
        if it.state == State::Completed {
            if let Some(ts) = it.resolved {
                *done_at.entry(period_label(ts, ctx.period)).or_insert(0) += 1;
            }
        }
    }
    if skipped > 0 {
        notes.push(format!(
            "{skipped} issue(s) have no created date and are left out of the burndown"
        ));
    }
    let mut labels: Vec<String> = created_at.keys().chain(done_at.keys()).cloned().collect();
    labels.sort();
    labels.dedup();
    let mut open = 0i64;
    let rows = labels
        .into_iter()
        .map(|label| {
            let c = *created_at.get(&label).unwrap_or(&0) as i64;
            let d = *done_at.get(&label).unwrap_or(&0) as i64;
            open += c - d;
            vec![
                Cell::S(label),
                Cell::I(c),
                Cell::I(d),
                Cell::I(c - d),
                Cell::I(open),
            ]
        })
        .collect();
    Table {
        key: "burndown".into(),
        title: match ctx.period {
            Period::Month => "Burndown by month".into(),
            Period::Day => "Burndown by day".into(),
            _ => "Burndown by week (week starting Monday)".to_string(),
        },
        columns: vec![
            ("period".into(), "Period".into()),
            ("created".into(), "Created".into()),
            ("completed".into(), "Completed".into()),
            ("net".into(), "Net".into()),
            ("open_at_end".into(), "Open at end".into()),
        ],
        rows,
    }
}

fn items_table(items: &[Item], ctx: &Ctx) -> Table {
    let u = ctx.unit.label();
    let mut columns = vec![
        ("key".to_string(), "Key".to_string()),
        ("status".into(), "Status".into()),
        ("state".into(), "State".into()),
    ];
    if ctx.has_points {
        columns.push(("points".to_string(), "Points".to_string()));
    }
    columns.push(("created".into(), "Created".into()));
    columns.push(("resolved".into(), "Resolved".into()));
    columns.push((format!("lead_{u}"), format!("Lead ({u})")));
    if ctx.has_started {
        columns.push((format!("cycle_{u}"), format!("Cycle ({u})")));
    }

    let mut rows: Vec<(f64, Vec<Cell>)> = items
        .iter()
        .map(|it| {
            let lead = match (it.created, it.resolved) {
                (Some(a), Some(b)) if it.state == State::Completed => {
                    Some(elapsed_secs(a, b, ctx.business_days) / ctx.unit.secs())
                }
                _ => None,
            };
            let cycle = match (it.started, it.resolved) {
                (Some(a), Some(b)) if it.state == State::Completed => {
                    Some(elapsed_secs(a, b, ctx.business_days) / ctx.unit.secs())
                }
                _ => None,
            };
            let mut r = vec![
                Cell::S(it.key.clone()),
                Cell::S(it.status.clone()),
                Cell::S(
                    match it.state {
                        State::Completed => "completed",
                        State::Cancelled => "cancelled",
                        State::Open => "open",
                    }
                    .into(),
                ),
            ];
            if ctx.has_points {
                r.push(it.points.map(Cell::N).unwrap_or(Cell::Null));
            }
            r.push(
                it.created
                    .map(|t| Cell::S(iso_date(t.div_euclid(86_400))))
                    .unwrap_or(Cell::Null),
            );
            r.push(
                it.resolved
                    .map(|t| Cell::S(iso_date(t.div_euclid(86_400))))
                    .unwrap_or(Cell::Null),
            );
            r.push(lead.map(Cell::N).unwrap_or(Cell::Null));
            if ctx.has_started {
                r.push(cycle.map(Cell::N).unwrap_or(Cell::Null));
            }
            (lead.unwrap_or(-1.0), r)
        })
        .collect();
    rows.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
    Table {
        key: "items".into(),
        title: format!("Issues, slowest first (lead time in {u})"),
        columns,
        rows: rows.into_iter().map(|(_, r)| r).collect(),
    }
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

fn render_text(out: &ReportOut) -> String {
    let mut s = String::new();
    for (i, t) in out.tables.iter().enumerate() {
        if i > 0 {
            s.push('\n');
        }
        let _ = writeln!(s, "{}", t.title);
        let _ = writeln!(s, "{}", "-".repeat(t.title.chars().count()));
        if t.rows.is_empty() {
            let _ = writeln!(s, "(nothing to report)");
            continue;
        }
        let headers: Vec<String> = t.columns.iter().map(|c| c.1.clone()).collect();
        let mut widths: Vec<usize> = headers.iter().map(|h| h.chars().count()).collect();
        let cells: Vec<Vec<String>> = t
            .rows
            .iter()
            .map(|r| r.iter().map(|c| c.text()).collect())
            .collect();
        for row in &cells {
            for (i, c) in row.iter().enumerate() {
                if i < widths.len() {
                    widths[i] = widths[i].max(c.chars().count());
                }
            }
        }
        let numeric: Vec<bool> = (0..headers.len())
            .map(|i| {
                t.rows
                    .iter()
                    .all(|r| r.get(i).map(|c| c.numeric()).unwrap_or(true))
            })
            .collect();
        let mut line = String::new();
        for (i, h) in headers.iter().enumerate() {
            if i > 0 {
                line.push_str("  ");
            }
            line.push_str(&pad(h, widths[i], numeric[i]));
        }
        let _ = writeln!(s, "{}", line.trim_end());
        for row in &cells {
            let mut line = String::new();
            for (i, c) in row.iter().enumerate() {
                if i > 0 {
                    line.push_str("  ");
                }
                line.push_str(&pad(c, widths[i], numeric[i]));
            }
            let _ = writeln!(s, "{}", line.trim_end());
        }
    }
    if !out.notes.is_empty() {
        s.push('\n');
        for n in &out.notes {
            let _ = writeln!(s, "note: {n}");
        }
    }
    s.trim_end().to_string()
}

fn pad(text: &str, width: usize, right: bool) -> String {
    let len = text.chars().count();
    let fill = " ".repeat(width.saturating_sub(len));
    if right {
        format!("{fill}{text}")
    } else {
        format!("{text}{fill}")
    }
}

fn md_escape(s: &str) -> String {
    s.replace('|', "\\|")
}

fn render_markdown(out: &ReportOut) -> String {
    let mut s = String::new();
    for t in &out.tables {
        let _ = writeln!(s, "## {}\n", t.title);
        if t.rows.is_empty() {
            let _ = writeln!(s, "_Nothing to report._\n");
            continue;
        }
        let _ = writeln!(
            s,
            "| {} |",
            t.columns
                .iter()
                .map(|c| md_escape(&c.1))
                .collect::<Vec<_>>()
                .join(" | ")
        );
        let _ = writeln!(
            s,
            "|{}|",
            t.columns
                .iter()
                .map(|_| " --- ")
                .collect::<Vec<_>>()
                .join("|")
        );
        for r in &t.rows {
            let _ = writeln!(
                s,
                "| {} |",
                r.iter()
                    .map(|c| md_escape(&c.text()))
                    .collect::<Vec<_>>()
                    .join(" | ")
            );
        }
        s.push('\n');
    }
    for n in &out.notes {
        let _ = writeln!(s, "> {n}");
    }
    s.trim_end().to_string()
}

fn render_csv(out: &ReportOut) -> String {
    let mut w = csv::WriterBuilder::new().from_writer(Vec::new());
    let multi = out.tables.len() > 1;
    for (i, t) in out.tables.iter().enumerate() {
        if multi {
            if i > 0 {
                let _ = w.write_record(std::iter::empty::<&str>());
            }
            let _ = w.write_record([format!("# {}", t.title)]);
        }
        let _ = w.write_record(t.columns.iter().map(|c| c.1.clone()));
        for r in &t.rows {
            let _ = w.write_record(r.iter().map(|c| c.plain()));
        }
    }
    for n in &out.notes {
        let _ = w.write_record([format!("# note: {n}")]);
    }
    let bytes = w.into_inner().unwrap_or_default();
    String::from_utf8(bytes)
        .unwrap_or_default()
        .trim_end()
        .to_string()
}

fn render_json(out: &ReportOut) -> String {
    let mut root = Map::new();
    root.insert("report".into(), Value::String(out.report.into()));
    root.insert("source".into(), Value::String(out.source.into()));
    let mut cols = Map::new();
    for (field, header) in &out.columns_used {
        cols.insert(field.clone(), Value::String(header.clone()));
    }
    root.insert("columns".into(), Value::Object(cols));
    let mut sections = Vec::new();
    for t in &out.tables {
        let mut sec = Map::new();
        sec.insert("key".into(), Value::String(t.key.clone()));
        sec.insert("title".into(), Value::String(t.title.clone()));
        let rows: Vec<Value> = t
            .rows
            .iter()
            .map(|r| {
                let mut o = Map::new();
                for (i, (k, _)) in t.columns.iter().enumerate() {
                    o.insert(k.clone(), r.get(i).map(|c| c.json()).unwrap_or(Value::Null));
                }
                Value::Object(o)
            })
            .collect();
        sec.insert("rows".into(), Value::Array(rows));
        sections.push(Value::Object(sec));
    }
    root.insert("sections".into(), Value::Array(sections));
    root.insert(
        "notes".into(),
        Value::Array(out.notes.iter().cloned().map(Value::String).collect()),
    );
    serde_json::to_string_pretty(&Value::Object(root)).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const DEFAULT_DONE: &str = "done,closed,resolved,completed,shipped,released,merged";
    const DEFAULT_CANCELLED: &str = "cancelled,canceled,won't do,wont do,duplicate,rejected,invalid";

    const JIRA: &str = "Issue key,Summary,Issue Type,Status,Priority,Assignee,Story Points,Created,Resolved,Sprint,Sprint\n\
GIZ-1,Login page,Story,Done,High,ana,3,05/Jan/24 9:00 AM,08/Jan/24 5:00 PM,Sprint 1,Sprint 1\n\
GIZ-2,Fix crash,Bug,Done,High,bo,2,05/Jan/24 9:00 AM,06/Jan/24 9:00 AM,Sprint 1,Sprint 1\n\
GIZ-3,Docs,Task,In Progress,Low,ana,1,06/Jan/24 9:00 AM,,Sprint 1,Sprint 2\n\
GIZ-4,Old idea,Task,Won't Do,Low,,5,04/Jan/24 9:00 AM,,,\n\
GIZ-5,Export,Story,Done,Medium,bo,8,08/Jan/24 9:00 AM,18/Jan/24 9:00 AM,Sprint 2,Sprint 2\n";

    fn report(data: &str, report: &str, format: &str) -> String {
        run(
            data,
            report,
            format,
            "auto",
            "",
            DEFAULT_DONE,
            DEFAULT_CANCELLED,
            "none",
            "auto",
            "days",
            false,
            "50,85,95",
        )
        .expect("report should render")
    }

    /// Collapse the text report's column padding so assertions read like the rows.
    fn squish(s: &str) -> String {
        s.split('\n')
            .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn jira_export_summary_counts_states() {
        let out = report(JIRA, "summary", "text");
        let sq = squish(&out);
        assert!(sq.contains("\nIssues 5\n"), "{out}");
        assert!(sq.contains("\nCompleted 3\n"), "{out}");
        assert!(sq.contains("\nOpen 1\n"), "{out}");
        assert!(sq.contains("\nCancelled 1\n"), "{out}");
        // 3 of 5 completed.
        assert!(sq.contains("Completion rate 60%"), "{out}");
        assert!(sq.contains("Done 3 60% 13"), "{out}");
        assert!(sq.contains("All issues 3 1 3.33 10 10 10 4.78"), "{out}");
        // Lead times: 3.33, 1, 10 days -> min 1, p50 3.33, max 10.
        assert!(out.contains("Lead time — created to resolved, in calendar days"));
        assert!(out.contains("Velocity by sprint"), "{out}");
    }

    #[test]
    fn status_report_csv_has_shares_and_points() {
        let out = report(JIRA, "status", "csv");
        assert!(out.starts_with("Status,Issues,Share,Points"), "{out}");
        assert!(out.contains("Done,3,60,13"), "{out}");
        assert!(out.contains("In Progress,1,20,1"), "{out}");
    }

    #[test]
    fn linear_export_is_detected_and_cycle_time_reported() {
        let csv = "ID,Title,Status,Assignee,Estimate,Created,Started,Completed,Cycle Name\n\
ENG-1,Ship it,Done,ana,3,2024-01-05T09:00:00Z,2024-01-06T09:00:00Z,2024-01-08T09:00:00Z,Cycle 4\n\
ENG-2,Bug,Done,bo,1,2024-01-05T09:00:00Z,2024-01-05T09:00:00Z,2024-01-07T09:00:00Z,Cycle 4\n\
ENG-3,Idea,Backlog,,2,2024-01-06T09:00:00Z,,,\n";
        let out = run(
            csv,
            "cycle_time",
            "json",
            "auto",
            "",
            DEFAULT_DONE,
            DEFAULT_CANCELLED,
            "none",
            "auto",
            "days",
            false,
            "50",
        )
        .unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["source"], "linear");
        assert_eq!(v["columns"]["started"], "Started");
        let lead = &v["sections"][0]["rows"][0];
        assert_eq!(lead["issues"], 2);
        assert_eq!(lead["min"], 2.0);
        assert_eq!(lead["max"], 3.0);
        let cycle = &v["sections"][1]["rows"][0];
        assert_eq!(cycle["min"], 2.0);
        assert_eq!(cycle["max"], 2.0);
    }

    #[test]
    fn business_days_skip_the_weekend() {
        // Fri 2024-01-05 09:00 -> Mon 2024-01-08 09:00 is 3 calendar days, 1 business day.
        let csv = "Key,Status,Created,Resolved\nA-1,Done,2024-01-05T09:00:00Z,2024-01-08T09:00:00Z\n";
        let calendar = run(
            csv, "cycle_time", "csv", "auto", "", DEFAULT_DONE, DEFAULT_CANCELLED, "none", "week",
            "days", false, "50",
        )
        .unwrap();
        assert!(calendar.contains("All issues,1,3,3,3,3"), "{calendar}");
        let business = run(
            csv, "cycle_time", "csv", "auto", "", DEFAULT_DONE, DEFAULT_CANCELLED, "none", "week",
            "days", true, "50",
        )
        .unwrap();
        assert!(business.contains("All issues,1,1,1,1,1"), "{business}");
    }

    #[test]
    fn velocity_by_sprint_totals_points() {
        let out = report(JIRA, "velocity", "csv");
        assert!(out.contains("Sprint 1,2,5"), "{out}");
        assert!(out.contains("Sprint 2,1,8"), "{out}");
        assert!(out.contains("Average issues per period,1.5"), "{out}");
    }

    #[test]
    fn burndown_tracks_open_backlog() {
        let out = run(
            JIRA, "burndown", "csv", "auto", "", DEFAULT_DONE, DEFAULT_CANCELLED, "none", "week",
            "days", false, "50",
        )
        .unwrap();
        // Weeks start Monday, so 04–06/Jan all land in the week of 2024-01-01
        // (4 created) and only GIZ-2 is resolved inside it -> 3 still open.
        assert!(out.contains("2024-01-01,4,1,3,3"), "{out}");
        // 08/Jan is itself a Monday: GIZ-5 arrives and GIZ-1 is resolved.
        assert!(out.contains("2024-01-08,1,1,0,3"), "{out}");
        // GIZ-5 resolved on 18/Jan, and nothing new arrives.
        assert!(out.contains("2024-01-15,0,1,-1,2"), "{out}");
    }

    #[test]
    fn group_by_assignee_splits_the_distribution() {
        let out = run(
            JIRA, "cycle_time", "csv", "auto", "", DEFAULT_DONE, DEFAULT_CANCELLED, "assignee",
            "auto", "days", false, "50",
        )
        .unwrap();
        assert!(out.starts_with("Assignee,Issues,Min,p50,Max,Mean"), "{out}");
        assert!(out.contains("bo,2,1,1,10,5.5"), "{out}");
        assert!(out.contains("ana,1,3.33,3.33,3.33,3.33"), "{out}");
    }

    #[test]
    fn column_override_maps_a_custom_header() {
        let csv = "Ticket,State,Opened,Shipped\nX-1,Live,2024-02-01,2024-02-05\n";
        let out = run(
            csv,
            "items",
            "csv",
            "auto",
            "status=State, created=Opened, resolved=Shipped, key=Ticket",
            "live",
            DEFAULT_CANCELLED,
            "none",
            "week",
            "days",
            false,
            "50",
        )
        .unwrap();
        assert!(out.contains("X-1,Live,completed,2024-02-01,2024-02-05,4"), "{out}");
    }

    #[test]
    fn hours_unit_and_custom_percentiles() {
        let csv = "Key,Status,Created,Resolved\nA-1,Done,2024-01-05T09:00:00Z,2024-01-05T21:00:00Z\n";
        let out = run(
            csv, "cycle_time", "csv", "auto", "", DEFAULT_DONE, DEFAULT_CANCELLED, "none", "week",
            "hours", false, "30,70",
        )
        .unwrap();
        assert!(out.starts_with("Scope,Issues,Min,p30,p70,Max,Mean"), "{out}");
        assert!(out.contains("All issues,1,12,12,12,12,12"), "{out}");
    }

    #[test]
    fn semicolon_delimiter_is_sniffed() {
        let csv = "Key;Status;Created;Resolved\nA-1;Done;2024-01-05;2024-01-07\n";
        let out = report(csv, "status", "csv");
        assert!(out.contains("Done,1,100"), "{out}");
    }

    #[test]
    fn cancelled_issues_are_excluded_from_velocity() {
        let csv = "Key,Status,Created,Resolved\n\
A-1,Done,2024-01-01,2024-01-02\n\
A-2,Duplicate,2024-01-01,2024-01-02\n";
        let out = run(
            csv, "velocity", "csv", "auto", "", DEFAULT_DONE, DEFAULT_CANCELLED, "none", "month",
            "days", false, "50",
        )
        .unwrap();
        assert!(out.contains("2024-01,1"), "{out}");
        assert!(out.contains("Completed issues,1"), "{out}");
    }

    #[test]
    fn bad_timestamp_is_reported_with_its_row() {
        let csv = "Key,Status,Created,Resolved\nA-1,Done,yesterday,2024-01-07\n";
        let err = run(
            csv, "summary", "text", "auto", "", DEFAULT_DONE, DEFAULT_CANCELLED, "none", "auto",
            "days", false, "50",
        )
        .unwrap_err();
        assert!(err.contains("row 2"), "{err}");
        assert!(err.contains("created timestamp 'yesterday'"), "{err}");
    }

    #[test]
    fn unknown_report_is_rejected() {
        let err = run(
            JIRA, "throughput", "text", "auto", "", DEFAULT_DONE, DEFAULT_CANCELLED, "none",
            "auto", "days", false, "50",
        )
        .unwrap_err();
        assert!(err.contains("unknown report 'throughput'"), "{err}");
    }

    #[test]
    fn percentile_out_of_range_is_rejected() {
        let err = run(
            JIRA, "cycle_time", "text", "auto", "", DEFAULT_DONE, DEFAULT_CANCELLED, "none",
            "auto", "days", false, "50,120",
        )
        .unwrap_err();
        assert!(err.contains("out of range"), "{err}");
    }

    #[test]
    fn unmappable_column_lists_the_headers() {
        let err = run(
            JIRA, "summary", "text", "auto", "status=Nope", DEFAULT_DONE, DEFAULT_CANCELLED,
            "none", "auto", "days", false, "50",
        )
        .unwrap_err();
        assert!(err.contains("no column named 'Nope'"), "{err}");
        assert!(err.contains("Issue key"), "{err}");
    }

    #[test]
    fn burndown_rejects_sprint_buckets() {
        let err = run(
            JIRA, "burndown", "text", "auto", "", DEFAULT_DONE, DEFAULT_CANCELLED, "none",
            "sprint", "days", false, "50",
        )
        .unwrap_err();
        assert!(err.contains("calendar buckets"), "{err}");
    }

    #[test]
    fn empty_input_is_rejected() {
        let err = run(
            "   ", "summary", "text", "auto", "", DEFAULT_DONE, DEFAULT_CANCELLED, "none", "auto",
            "days", false, "50",
        )
        .unwrap_err();
        assert!(err.contains("no CSV data"), "{err}");
    }

    #[test]
    fn row_cap_boundary() {
        let mut at = String::from("Key,Status,Created,Resolved\n");
        for i in 0..MAX_ROWS {
            at.push_str(&format!("A-{i},Done,2024-01-01,2024-01-02\n"));
        }
        let ok = run(
            &at, "status", "csv", "auto", "", DEFAULT_DONE, DEFAULT_CANCELLED, "none", "month",
            "days", false, "50",
        )
        .unwrap();
        assert!(ok.contains(&format!("Done,{MAX_ROWS},100")), "{ok}");
        let mut over = at.clone();
        over.push_str("A-over,Done,2024-01-01,2024-01-02\n");
        let err = run(
            &over, "status", "csv", "auto", "", DEFAULT_DONE, DEFAULT_CANCELLED, "none", "month",
            "days", false, "50",
        )
        .unwrap_err();
        assert!(err.contains(&format!("more than {MAX_ROWS} issue rows")), "{err}");
    }

    #[test]
    fn oversize_input_is_rejected() {
        let big = "x".repeat(MAX_INPUT_BYTES + 1);
        let err = run(
            &big, "summary", "text", "auto", "", DEFAULT_DONE, DEFAULT_CANCELLED, "none", "auto",
            "days", false, "50",
        )
        .unwrap_err();
        assert!(err.contains("over the"), "{err}");
    }

    #[test]
    fn jira_and_iso_timestamps_agree() {
        assert_eq!(parse_ts("05/Jan/24 9:00 AM").unwrap(), parse_ts("2024-01-05T09:00:00Z").unwrap());
        assert_eq!(parse_ts("05/Jan/24 12:00 AM").unwrap(), parse_ts("2024-01-05").unwrap());
        assert_eq!(parse_ts("2024-01-05T09:00:00+02:00").unwrap(), parse_ts("2024-01-05T07:00:00Z").unwrap());
        assert_eq!(parse_ts("2024-01-05 09:00:00.123+0000").unwrap(), parse_ts("2024-01-05T09:00:00Z").unwrap());
        assert!(parse_ts("").unwrap().is_none());
        assert!(parse_ts("not a date").is_err());
    }

    #[test]
    fn blank_status_lists_fall_back_to_the_defaults() {
        // Same export, once with the lists spelled out and once with them blank.
        let spelled = report(JIRA, "summary", "csv");
        let blank = run(
            JIRA, "summary", "csv", "auto", "", "", "", "none", "auto", "days", false, "",
        )
        .unwrap();
        assert_eq!(spelled, blank);
        assert!(blank.contains("Cancelled,1"), "{blank}");
    }

    #[test]
    fn markdown_output_has_a_table() {
        let out = report(JIRA, "status", "markdown");
        assert!(out.starts_with("## Status breakdown"), "{out}");
        assert!(out.contains("| Status | Issues | Share | Points |"), "{out}");
    }
}
