//! gizza-ai/super-timeline-builder — pure core.
//!
//! Merges the CSV output of several *already-parsed* forensic artifacts (an MFT
//! listing, an event-log export, a prefetch/registry/browser-history table, …)
//! into ONE chronologically sorted super-timeline.
//!
//! Input is a single pasted blob holding one CSV per artifact, each introduced
//! by a header line that names the source:
//!   `--- mft.csv ---` · `=== evtx ===` · `==> prefetch <==` (GNU tail) · `# mft`
//! A blob with no header line is treated as one section named `artifact1`.
//!
//! Every section keeps its own column layout. For each one the parser:
//!   1. auto-detects the **delimiter** (comma / tab / semicolon / pipe),
//!   2. reads the first row as the **header**,
//!   3. classifies every column: timestamp columns (by time-ish header name, or
//!      by ≥80 % of values parsing as an unambiguous date), plus the optional
//!      `macb` / `user` / `host` / `filename` / `inode` / description columns,
//!   4. emits one event per parseable timestamp CELL (`expand = true`), the way
//!      a super-timeline expands an MFT row into its Created / Modified /
//!      Accessed lines, each tagged with the timestamp column's name.
//!
//! Events from every section are then normalized to UTC, optionally range- and
//! duplicate-filtered, sorted, and rendered as `csv` (compact), `l2tcsv` (the
//! 17-field legacy layout) or `tln` (pipe-delimited `Time|Source|Host|User|
//! Description`).
//!
//! No system clock is read — only parsing/formatting — so this crate stays
//! instantiable under both wafer (wasm32-wasip1) and wasm-pack
//! (wasm32-unknown-unknown) with no getrandom/js dependency.

use chrono::{DateTime, NaiveDate, NaiveDateTime, TimeZone, Utc};
use std::collections::HashSet;

/// Hard cap on total input lines (across all sections).
pub const MAX_LINES: usize = 200_000;
/// Hard upper bound the `limit` param is clamped/validated against.
pub const MAX_LIMIT: u32 = 100_000;
/// Characters kept in the l2tcsv `short` field.
const SHORT_LEN: usize = 80;

// ---------------------------------------------------------------------------
// One normalized timeline event.
// ---------------------------------------------------------------------------

/// A single timestamped row of the merged timeline (times are epoch millis, UTC).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Event {
    pub millis: i64,
    /// Which timestamp this is — the timestamp column's name, or the value of
    /// an explicit `timestamp_desc` / `type` column.
    pub desc: String,
    /// The artifact this row came from (the section's header name).
    pub source: String,
    pub macb: String,
    pub user: String,
    pub host: String,
    pub filename: String,
    pub inode: String,
    pub message: String,
}

// ---------------------------------------------------------------------------
// Timestamp parsing → epoch millis (UTC).
// ---------------------------------------------------------------------------

/// Naive (timezone-less) layouts we accept, most specific first.
const NAIVE_DATETIME_FORMATS: &[&str] = &[
    "%Y-%m-%dT%H:%M:%S%.f",
    "%Y-%m-%d %H:%M:%S%.f",
    "%Y-%m-%dT%H:%M",
    "%Y-%m-%d %H:%M",
    "%Y/%m/%d %H:%M:%S%.f",
    "%m/%d/%Y %H:%M:%S%.f",
    "%m/%d/%Y %H:%M",
];
const NAIVE_DATE_FORMATS: &[&str] = &["%Y-%m-%d", "%Y/%m/%d", "%m/%d/%Y"];

/// Parse one cell into epoch millis (UTC).
///
/// Absolute values (RFC 3339 / trailing `Z` or `±hh:mm`) are used as-is; naive
/// values are read as local time in `offset_secs` and shifted to UTC. Bare
/// integers are only read as epoch seconds/millis/micros when `allow_epoch` is
/// set (a column of inode numbers must never become a timeline).
pub fn parse_timestamp(raw: &str, allow_epoch: bool, offset_secs: i64) -> Option<i64> {
    let s = raw.trim().trim_matches('"').trim();
    if s.is_empty() {
        return None;
    }
    // Absolute: RFC 3339 (optionally with a space instead of the "T").
    let rfc = if s.len() > 10 && s.as_bytes()[10] == b' ' {
        let mut t = s.to_string();
        t.replace_range(10..11, "T");
        t
    } else {
        s.to_string()
    };
    if let Ok(dt) = DateTime::parse_from_rfc3339(&rfc) {
        return Some(dt.timestamp_millis());
    }
    // Naive date-time, then date-only (midnight).
    for f in NAIVE_DATETIME_FORMATS {
        if let Ok(dt) = NaiveDateTime::parse_from_str(s, f) {
            return Some(dt.and_utc().timestamp_millis() - offset_secs * 1000);
        }
    }
    for f in NAIVE_DATE_FORMATS {
        if let Ok(d) = NaiveDate::parse_from_str(s, f) {
            return Some(
                d.and_hms_opt(0, 0, 0)?.and_utc().timestamp_millis() - offset_secs * 1000,
            );
        }
    }
    // Epoch seconds / milliseconds / microseconds.
    if allow_epoch {
        let digits = s.strip_prefix('-').unwrap_or(s);
        let whole = digits.split('.').next().unwrap_or(digits);
        if !whole.is_empty() && whole.bytes().all(|b| b.is_ascii_digit()) {
            let secs: f64 = s.parse().ok()?;
            return Some(match whole.len() {
                1..=11 => (secs * 1000.0) as i64,
                12..=14 => secs as i64,
                15..=17 => (secs / 1000.0) as i64,
                _ => return None,
            });
        }
    }
    None
}

/// True when a cell looks like a bare clock time (`HH:MM:SS[.fff]`).
fn is_clock_time(s: &str) -> bool {
    let s = s.trim();
    let mut parts = s.split(':');
    let (h, m) = match (parts.next(), parts.next()) {
        (Some(h), Some(m)) => (h, m),
        _ => return false,
    };
    let sec = parts.next().unwrap_or("0");
    if parts.next().is_some() {
        return false;
    }
    let sec = sec.split('.').next().unwrap_or(sec);
    h.len() <= 2
        && !h.is_empty()
        && h.bytes().all(|b| b.is_ascii_digit())
        && m.len() == 2
        && m.bytes().all(|b| b.is_ascii_digit())
        && !sec.is_empty()
        && sec.bytes().all(|b| b.is_ascii_digit())
        && h.parse::<u32>().map(|v| v < 24).unwrap_or(false)
}

/// Format epoch millis as RFC 3339 in UTC (`.mmm` only when non-zero).
fn fmt_iso(millis: i64) -> String {
    let dt = Utc.timestamp_millis_opt(millis).single().unwrap_or_else(|| Utc.timestamp_nanos(0));
    if millis.rem_euclid(1000) == 0 {
        dt.format("%Y-%m-%dT%H:%M:%SZ").to_string()
    } else {
        dt.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
    }
}

// ---------------------------------------------------------------------------
// Section splitting.
// ---------------------------------------------------------------------------

/// A named artifact section: the header-line name plus its raw CSV body.
struct Section {
    name: String,
    body: String,
}

/// Recognize `--- name ---`, `=== name ===`, `==> name <==`, `# name`.
fn section_name(line: &str) -> Option<String> {
    let t = line.trim();
    if t.is_empty() {
        return None;
    }
    let inner = if let Some(r) = t.strip_prefix("==>").and_then(|r| r.strip_suffix("<==")) {
        r
    } else if t.starts_with("---") && t.ends_with("---") && t.len() > 6 {
        &t[3..t.len() - 3]
    } else if t.starts_with("===") && t.ends_with("===") && t.len() > 6 {
        &t[3..t.len() - 3]
    } else if let Some(r) = t.strip_prefix('#') {
        // `# name` — a comment-style header, never a CSV data row.
        r
    } else {
        return None;
    };
    let name = inner.trim().trim_matches('#').trim();
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

fn split_sections(input: &str) -> Vec<Section> {
    let mut out: Vec<Section> = Vec::new();
    let mut cur: Option<Section> = None;
    for line in input.lines() {
        if let Some(name) = section_name(line) {
            if let Some(s) = cur.take() {
                out.push(s);
            }
            cur = Some(Section { name, body: String::new() });
            continue;
        }
        match cur.as_mut() {
            Some(s) => {
                s.body.push_str(line);
                s.body.push('\n');
            }
            None => {
                if !line.trim().is_empty() {
                    cur = Some(Section {
                        name: "artifact1".to_string(),
                        body: format!("{line}\n"),
                    });
                }
            }
        }
    }
    if let Some(s) = cur.take() {
        out.push(s);
    }
    out.retain(|s| !s.body.trim().is_empty());
    out
}

// ---------------------------------------------------------------------------
// Column classification.
// ---------------------------------------------------------------------------

/// Lowercase, alphanumerics only — `Last Write Time` → `lastwritetime`.
fn norm(name: &str) -> String {
    name.chars().filter(|c| c.is_alphanumeric()).flat_map(|c| c.to_lowercase()).collect()
}

const TIMEISH: &[&str] = &[
    "time", "date", "created", "modif", "access", "written", "changed", "born", "stamp", "utc",
    "epoch", "crtime", "btime",
];

fn is_timeish(normalized: &str) -> bool {
    TIMEISH.iter().any(|k| normalized.contains(k))
}

fn delim_byte(d: &str, body: &str) -> Result<u8, String> {
    Ok(match d.trim().to_ascii_lowercase().as_str() {
        "" | "auto" => detect_delim(body),
        "," | "comma" => b',',
        "\t" | "tab" | "\\t" => b'\t',
        ";" | "semicolon" => b';',
        "|" | "pipe" => b'|',
        other => {
            return Err(format!(
                "delimiter must be auto, comma, tab, semicolon or pipe — got '{other}'"
            ))
        }
    })
}

/// Pick the candidate delimiter that occurs most often in the header line.
fn detect_delim(body: &str) -> u8 {
    let head = body.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
    let mut best = (b',', 0usize);
    for c in [b',', b'\t', b';', b'|'] {
        let n = head.bytes().filter(|b| *b == c).count();
        if n > best.1 {
            best = (c, n);
        }
    }
    best.0
}

/// Which column plays which role in one section.
struct Layout {
    /// (column index, label) for every timestamp column, in header order.
    ts: Vec<(usize, String)>,
    /// A date-only column paired with a `time` column (rendered together).
    time_pair: Option<(usize, usize)>,
    desc_col: Option<usize>,
    macb: Option<usize>,
    user: Option<usize>,
    host: Option<usize>,
    filename: Option<usize>,
    inode: Option<usize>,
    message: Option<usize>,
}

fn find_col(headers: &[String], names: &[&str]) -> Option<usize> {
    headers.iter().position(|h| names.contains(&h.as_str()))
}

fn classify(
    headers: &[String],
    rows: &[csv::StringRecord],
    offset_secs: i64,
) -> Layout {
    let n = headers.len();
    let mut ts: Vec<(usize, String)> = Vec::new();

    // `date` + `time` split across two columns (l2tcsv, mactime).
    let date_i = headers.iter().position(|h| h == "date");
    let time_i = headers.iter().position(|h| h == "time");
    let mut time_pair = None;
    if let (Some(d), Some(t)) = (date_i, time_i) {
        let dates_ok = fraction(rows, d, |v| {
            NAIVE_DATE_FORMATS.iter().any(|f| NaiveDate::parse_from_str(v.trim(), f).is_ok())
        });
        let times_ok = fraction(rows, t, is_clock_time);
        if dates_ok >= 0.5 && times_ok >= 0.5 {
            time_pair = Some((d, t));
        }
    }

    for i in 0..n {
        if matches!(time_pair, Some((_, t)) if t == i) {
            continue; // consumed by its date column
        }
        let timeish = is_timeish(&headers[i]);
        let paired = matches!(time_pair, Some((d, _)) if d == i);
        let rate = if paired {
            1.0
        } else {
            fraction(rows, i, |v| parse_timestamp(v, timeish, offset_secs).is_some())
        };
        let keep = if timeish || paired { rate >= 0.5 } else { rate >= 0.8 };
        if keep {
            ts.push((i, headers[i].clone()));
        }
    }

    Layout {
        ts,
        time_pair,
        desc_col: find_col(headers, &["timestampdesc", "type"]),
        macb: find_col(headers, &["macb"]),
        user: find_col(headers, &["user", "username", "useraccount", "account"]),
        host: find_col(headers, &["host", "hostname", "computer", "computername", "machine"]),
        filename: find_col(headers, &["filename", "file", "path", "filepath", "fullpath", "name"]),
        inode: find_col(headers, &["inode", "entrynumber", "recordnumber", "eventrecordid"]),
        message: find_col(
            headers,
            &["message", "desc", "description", "details", "payload", "short"],
        ),
    }
}

/// Fraction of the non-empty cells in column `i` that satisfy `pred`.
fn fraction(rows: &[csv::StringRecord], i: usize, pred: impl Fn(&str) -> bool) -> f64 {
    let mut total = 0usize;
    let mut hit = 0usize;
    for r in rows {
        let v = r.get(i).unwrap_or("").trim();
        if v.is_empty() {
            continue;
        }
        total += 1;
        if pred(v) {
            hit += 1;
        }
    }
    if total == 0 {
        0.0
    } else {
        hit as f64 / total as f64
    }
}

fn cell(r: &csv::StringRecord, i: Option<usize>) -> String {
    i.and_then(|i| r.get(i)).unwrap_or("").trim().to_string()
}

// ---------------------------------------------------------------------------
// Section → events.
// ---------------------------------------------------------------------------

fn section_events(
    sec: &Section,
    delimiter: &str,
    expand: bool,
    offset_secs: i64,
    out: &mut Vec<Event>,
) -> Result<(), String> {
    let delim = delim_byte(delimiter, &sec.body)?;
    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(delim)
        .has_headers(false)
        .flexible(true)
        .trim(csv::Trim::All)
        .from_reader(sec.body.as_bytes());
    let records: Vec<csv::StringRecord> = rdr
        .records()
        .collect::<Result<_, _>>()
        .map_err(|e| format!("section '{}': CSV parse error: {e}", sec.name))?;
    if records.len() < 2 {
        return Err(format!(
            "section '{}': expected a header row plus at least 1 data row, got {} row(s)",
            sec.name,
            records.len()
        ));
    }
    let headers: Vec<String> = records[0].iter().map(norm).collect();
    let raw_headers: Vec<String> = records[0].iter().map(|h| h.trim().to_string()).collect();
    let rows = &records[1..];
    let layout = classify(&headers, rows, offset_secs);
    if layout.ts.is_empty() {
        return Err(format!(
            "section '{}': no timestamp column found — expected a column whose header names a time (Created, LastWriteTime, datetime, …) or whose values are ISO 8601 dates; header was: {}",
            sec.name,
            raw_headers.join(", ")
        ));
    }
    let ts_cols: Vec<&(usize, String)> =
        if expand { layout.ts.iter().collect() } else { layout.ts.iter().take(1).collect() };
    let ts_idx: HashSet<usize> = layout.ts.iter().map(|(i, _)| *i).collect();

    for r in rows {
        let macb = cell(r, layout.macb);
        let user = cell(r, layout.user);
        let host = cell(r, layout.host);
        let filename = cell(r, layout.filename);
        let inode = cell(r, layout.inode);
        let message = match layout.message {
            Some(i) => cell(r, Some(i)),
            None => {
                let mut parts: Vec<String> = Vec::new();
                for (i, raw) in raw_headers.iter().enumerate() {
                    if ts_idx.contains(&i) || matches!(layout.time_pair, Some((_, t)) if t == i) {
                        continue;
                    }
                    let v = r.get(i).unwrap_or("").trim();
                    if !v.is_empty() {
                        parts.push(format!("{raw}={v}"));
                    }
                }
                parts.join("; ")
            }
        };
        for (i, label) in &ts_cols {
            let raw = match layout.time_pair {
                Some((d, t)) if d == *i => {
                    format!("{} {}", r.get(d).unwrap_or("").trim(), r.get(t).unwrap_or("").trim())
                }
                _ => r.get(*i).unwrap_or("").trim().to_string(),
            };
            let millis = match parse_timestamp(&raw, is_timeish(&headers[*i]), offset_secs) {
                Some(m) => m,
                None => continue,
            };
            let desc = match layout.desc_col {
                Some(d) if !cell(r, Some(d)).is_empty() && layout.ts.len() == 1 => cell(r, Some(d)),
                _ => raw_headers.get(*i).cloned().unwrap_or_else(|| label.clone()),
            };
            out.push(Event {
                millis,
                desc,
                source: sec.name.clone(),
                macb: macb.clone(),
                user: user.clone(),
                host: host.clone(),
                filename: filename.clone(),
                inode: inode.clone(),
                message: message.clone(),
            });
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Rendering.
// ---------------------------------------------------------------------------

fn truncate_chars(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        s.chars().take(n).collect()
    }
}

fn render(events: &[Event], format: &str) -> Result<String, String> {
    let (delim, headers): (u8, Vec<&str>) = match format {
        "csv" => (b',', vec!["datetime", "timestamp_desc", "source", "message"]),
        "l2tcsv" => (
            b',',
            vec![
                "date", "time", "timezone", "MACB", "source", "sourcetype", "type", "user", "host",
                "short", "desc", "version", "filename", "inode", "notes", "format", "extra",
            ],
        ),
        "tln" => (b'|', vec!["Time", "Source", "Host", "User", "Description"]),
        other => {
            return Err(format!("format must be csv, l2tcsv or tln — got '{other}'"));
        }
    };
    let mut w = csv::WriterBuilder::new().delimiter(delim).from_writer(vec![]);
    w.write_record(&headers).map_err(|e| e.to_string())?;
    for e in events {
        let dt = Utc.timestamp_millis_opt(e.millis).single();
        match format {
            "csv" => w.write_record([&fmt_iso(e.millis), &e.desc, &e.source, &e.message]),
            "l2tcsv" => {
                let (date, time) = match dt {
                    Some(d) => {
                        (d.format("%m/%d/%Y").to_string(), d.format("%H:%M:%S").to_string())
                    }
                    None => (String::new(), String::new()),
                };
                let src_short = e.source.to_uppercase();
                let short = truncate_chars(&e.message, SHORT_LEN);
                w.write_record([
                    date.as_str(),
                    time.as_str(),
                    "UTC",
                    e.macb.as_str(),
                    src_short.as_str(),
                    e.source.as_str(),
                    e.desc.as_str(),
                    e.user.as_str(),
                    e.host.as_str(),
                    short.as_str(),
                    e.message.as_str(),
                    "2",
                    e.filename.as_str(),
                    e.inode.as_str(),
                    "",
                    e.source.as_str(),
                    "",
                ])
            }
            _ => w.write_record([
                &(e.millis.div_euclid(1000)).to_string(),
                &e.source,
                &e.host,
                &e.user,
                &format!("{} - {}", e.desc, e.message),
            ]),
        }
        .map_err(|e| e.to_string())?;
    }
    let bytes = w.into_inner().map_err(|e| e.to_string())?;
    String::from_utf8(bytes).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Public entry point.
// ---------------------------------------------------------------------------

/// Merge every artifact section in `artifacts` into one sorted super-timeline.
///
/// * `format` — `csv` (compact) · `l2tcsv` (17-field legacy) · `tln` (pipe).
/// * `order` — `asc` (oldest first) or `desc`.
/// * `expand` — one row per timestamp COLUMN (MFT Created/Modified/Accessed …)
///   instead of only the first one.
/// * `dedupe` — drop repeats of an identical (time, source, type, message).
/// * `from` / `to` — inclusive UTC bounds; empty = unbounded.
/// * `tz_offset` — hours (may be fractional) that timezone-less input times are
///   offset from UTC; values carrying `Z`/`±hh:mm` are unaffected.
/// * `drop_epoch_zero` — drop rows landing exactly on 1970-01-01T00:00:00Z.
/// * `delimiter` — `auto` (per section) · comma · tab · semicolon · pipe.
/// * `limit` — maximum rows; exceeding it is an error, never a silent trim.
#[allow(clippy::too_many_arguments)]
pub fn build(
    artifacts: &str,
    format: &str,
    order: &str,
    expand: bool,
    dedupe: bool,
    from: &str,
    to: &str,
    tz_offset: f64,
    drop_epoch_zero: bool,
    delimiter: &str,
    limit: u32,
) -> Result<String, String> {
    if artifacts.trim().is_empty() {
        return Err("artifacts is empty — paste one or more artifact CSVs, each under a header line like `--- mft.csv ---`".into());
    }
    if !matches!(format, "csv" | "l2tcsv" | "tln") {
        return Err(format!("format must be csv, l2tcsv or tln — got '{format}'"));
    }
    if !matches!(order, "asc" | "desc") {
        return Err(format!("order must be asc or desc — got '{order}'"));
    }
    if !(-14.0..=14.0).contains(&tz_offset) {
        return Err(format!("tz_offset must be between -14 and 14 hours — got {tz_offset}"));
    }
    if limit == 0 || limit > MAX_LIMIT {
        return Err(format!("limit must be between 1 and {MAX_LIMIT} — got {limit}"));
    }
    let lines = artifacts.lines().count();
    if lines > MAX_LINES {
        return Err(format!("input has {lines} lines, above the {MAX_LINES}-line cap"));
    }
    let offset_secs = (tz_offset * 3600.0).round() as i64;

    let from_ms = if from.trim().is_empty() {
        None
    } else {
        Some(parse_timestamp(from, true, offset_secs).ok_or_else(|| {
            format!("could not parse from='{from}' — expected a date like 2024-06-01 or 2024-06-01T10:00:00Z")
        })?)
    };
    let to_ms = if to.trim().is_empty() {
        None
    } else {
        Some(parse_timestamp(to, true, offset_secs).ok_or_else(|| {
            format!("could not parse to='{to}' — expected a date like 2024-06-02 or 2024-06-02T23:59:59Z")
        })?)
    };
    if let (Some(a), Some(b)) = (from_ms, to_ms) {
        if a > b {
            return Err("from is later than to — swap the range bounds".into());
        }
    }

    let sections = split_sections(artifacts);
    if sections.is_empty() {
        return Err("no artifact sections found — paste at least one CSV".into());
    }
    let mut events: Vec<Event> = Vec::new();
    for sec in &sections {
        section_events(sec, delimiter, expand, offset_secs, &mut events)?;
    }
    if events.is_empty() {
        return Err("no timestamped events found in any section".into());
    }

    events.retain(|e| {
        if drop_epoch_zero && e.millis == 0 {
            return false;
        }
        if let Some(a) = from_ms {
            if e.millis < a {
                return false;
            }
        }
        if let Some(b) = to_ms {
            if e.millis > b {
                return false;
            }
        }
        true
    });
    if events.is_empty() {
        return Err("every event was filtered out — widen from/to or turn off drop_epoch_zero".into());
    }

    if order == "asc" {
        events.sort_by_key(|e| e.millis);
    } else {
        events.sort_by_key(|e| std::cmp::Reverse(e.millis));
    }

    if dedupe {
        let mut seen: HashSet<(i64, String, String, String)> = HashSet::new();
        events.retain(|e| {
            seen.insert((e.millis, e.source.clone(), e.desc.clone(), e.message.clone()))
        });
    }

    if events.len() > limit as usize {
        return Err(format!(
            "timeline has {} events, above the limit of {limit} — raise limit (max {MAX_LIMIT}) or narrow from/to",
            events.len()
        ));
    }
    render(&events, format)
}

// ---------------------------------------------------------------------------
// Tests.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const TWO: &str = "--- mft ---\nPath,Created,LastModified\n\\Users\\a\\evil.exe,2024-06-01 10:00:05,2024-06-01 10:00:09\n=== evtx ===\nTimeCreated,EventID,Computer\n2024-06-01 10:00:01,4624,DC01\n";

    fn run(a: &str) -> String {
        build(a, "csv", "asc", true, true, "", "", 0.0, false, "auto", 10_000).unwrap()
    }

    #[test]
    fn merges_two_artifacts_into_one_sorted_timeline() {
        let out = run(TWO);
        assert_eq!(
            out,
            "datetime,timestamp_desc,source,message\n\
             2024-06-01T10:00:01Z,TimeCreated,evtx,EventID=4624; Computer=DC01\n\
             2024-06-01T10:00:05Z,Created,mft,Path=\\Users\\a\\evil.exe\n\
             2024-06-01T10:00:09Z,LastModified,mft,Path=\\Users\\a\\evil.exe\n"
        );
    }

    #[test]
    fn expand_off_keeps_only_the_first_timestamp_column() {
        let out =
            build(TWO, "csv", "asc", false, true, "", "", 0.0, false, "auto", 10_000).unwrap();
        assert_eq!(out.lines().count(), 3, "header + 1 row per section");
        assert!(!out.contains("LastModified"), "second MFT timestamp is dropped: {out}");
    }

    #[test]
    fn desc_order_puts_newest_first() {
        let out =
            build(TWO, "csv", "desc", true, true, "", "", 0.0, false, "auto", 10_000).unwrap();
        assert!(out.lines().nth(1).unwrap().starts_with("2024-06-01T10:00:09Z"), "{out}");
    }

    #[test]
    fn range_filter_is_inclusive_on_both_bounds() {
        let out = build(
            TWO,
            "csv",
            "asc",
            true,
            true,
            "2024-06-01T10:00:05Z",
            "2024-06-01T10:00:09Z",
            0.0,
            false,
            "auto",
            10_000,
        )
        .unwrap();
        assert_eq!(out.lines().count(), 3, "{out}");
        assert!(!out.contains("TimeCreated"), "{out}");
    }

    #[test]
    fn tz_offset_shifts_naive_times_to_utc() {
        let out = build(
            "# evtx\nTimeCreated,EventID\n2024-06-01 12:00:00,4624\n",
            "csv",
            "asc",
            true,
            true,
            "",
            "",
            2.0,
            false,
            "auto",
            10_000,
        )
        .unwrap();
        assert!(out.contains("2024-06-01T10:00:00Z"), "{out}");
    }

    #[test]
    fn absolute_offsets_in_the_data_are_respected() {
        let out = run("# evtx\nTimeCreated,EventID\n2024-06-01T12:00:00+02:00,4624\n");
        assert!(out.contains("2024-06-01T10:00:00Z"), "{out}");
    }

    #[test]
    fn epoch_seconds_and_millis_are_read_from_time_named_columns() {
        let out = run("# a\nepoch,note\n1717236001,x\n1717236002000,y\n");
        assert!(out.contains("2024-06-01T10:00:01Z") && out.contains("2024-06-01T10:00:02Z"), "{out}");
    }

    #[test]
    fn bare_integer_columns_are_not_mistaken_for_timestamps() {
        let err = build(
            "# mft\nEntryNumber,Size\n1717236001,4096\n",
            "csv",
            "asc",
            true,
            true,
            "",
            "",
            0.0,
            false,
            "auto",
            10_000,
        )
        .unwrap_err();
        assert!(err.contains("no timestamp column"), "{err}");
    }

    #[test]
    fn split_date_and_time_columns_are_combined() {
        let out = run("# l2t\ndate,time,timezone,MACB,source,desc\n06/01/2024,10:00:07,UTC,MACB,FILE,evil.exe\n");
        assert!(out.contains("2024-06-01T10:00:07Z"), "{out}");
        assert!(out.contains("evil.exe"), "{out}");
    }

    #[test]
    fn l2tcsv_output_has_the_17_legacy_fields() {
        let out =
            build(TWO, "l2tcsv", "asc", true, true, "", "", 0.0, false, "auto", 10_000).unwrap();
        let head: Vec<&str> = out.lines().next().unwrap().split(',').collect();
        assert_eq!(head.len(), 17, "{out}");
        assert_eq!(head[0], "date");
        assert_eq!(head[3], "MACB");
        assert_eq!(head[16], "extra");
        assert!(out.contains("06/01/2024,10:00:01,UTC,,EVTX,evtx,TimeCreated,,DC01,"), "{out}");
    }

    #[test]
    fn tln_output_is_pipe_delimited_epoch_seconds() {
        let out =
            build(TWO, "tln", "asc", true, true, "", "", 0.0, false, "auto", 10_000).unwrap();
        assert_eq!(out.lines().next().unwrap(), "Time|Source|Host|User|Description");
        assert!(out.contains("1717236001|evtx|DC01||TimeCreated - "), "{out}");
    }

    #[test]
    fn dedupe_drops_identical_events_across_overlapping_exports() {
        let dup = "# a\nTimeCreated,EventID\n2024-06-01 10:00:01,4624\n# a\nTimeCreated,EventID\n2024-06-01 10:00:01,4624\n";
        let on = run(dup);
        let off =
            build(dup, "csv", "asc", false, false, "", "", 0.0, false, "auto", 10_000).unwrap();
        assert_eq!(on.lines().count(), 2, "{on}");
        assert_eq!(off.lines().count(), 3, "{off}");
    }

    #[test]
    fn drop_epoch_zero_removes_null_1970_timestamps() {
        let src = "# mft\nPath,Created\na.txt,1970-01-01 00:00:00\nb.txt,2024-06-01 10:00:01\n";
        assert!(run(src).contains("1970-01-01T00:00:00Z"));
        let out =
            build(src, "csv", "asc", true, true, "", "", 0.0, true, "auto", 10_000).unwrap();
        assert!(!out.contains("1970"), "{out}");
    }

    #[test]
    fn tab_and_pipe_delimiters_are_auto_detected() {
        let out = run("# a\nTimeCreated\tEventID\n2024-06-01 10:00:01\t4624\n");
        assert!(out.contains("EventID=4624"), "{out}");
        let out = run("# a\nTimeCreated|EventID\n2024-06-01 10:00:01|4624\n");
        assert!(out.contains("EventID=4624"), "{out}");
    }

    #[test]
    fn a_named_description_column_becomes_the_message() {
        let out = run("# evtx\nTimeCreated,Message,EventID\n2024-06-01 10:00:01,User logon,4624\n");
        assert!(out.ends_with("2024-06-01T10:00:01Z,TimeCreated,evtx,User logon\n"), "{out}");
    }

    #[test]
    fn headerless_blob_is_treated_as_one_artifact() {
        let out = run("TimeCreated,EventID\n2024-06-01 10:00:01,4624\n");
        assert!(out.contains(",artifact1,"), "{out}");
    }

    #[test]
    fn limit_overflow_is_an_error_not_a_silent_trim() {
        let err =
            build(TWO, "csv", "asc", true, true, "", "", 0.0, false, "auto", 2).unwrap_err();
        assert!(err.contains("above the limit of 2"), "{err}");
    }

    #[test]
    fn empty_input_is_rejected_with_guidance() {
        let err =
            build("   ", "csv", "asc", true, true, "", "", 0.0, false, "auto", 10_000).unwrap_err();
        assert!(err.contains("artifacts is empty"), "{err}");
    }

    #[test]
    fn bad_enum_values_name_the_accepted_choices() {
        let e1 = build(TWO, "xlsx", "asc", true, true, "", "", 0.0, false, "auto", 10).unwrap_err();
        assert!(e1.contains("csv, l2tcsv or tln"), "{e1}");
        let e2 = build(TWO, "csv", "up", true, true, "", "", 0.0, false, "auto", 10).unwrap_err();
        assert!(e2.contains("asc or desc"), "{e2}");
        let e3 =
            build(TWO, "csv", "asc", true, true, "", "", 99.0, false, "auto", 10).unwrap_err();
        assert!(e3.contains("between -14 and 14"), "{e3}");
        let e4 = build(TWO, "csv", "asc", true, true, "", "", 0.0, false, "auto", 0).unwrap_err();
        assert!(e4.contains("between 1 and 100000"), "{e4}");
        let e5 =
            build(TWO, "csv", "asc", true, true, "yesterday", "", 0.0, false, "auto", 10)
                .unwrap_err();
        assert!(e5.contains("could not parse from='yesterday'"), "{e5}");
    }

    #[test]
    fn a_section_without_data_rows_names_itself_in_the_error() {
        let err = build(
            "--- mft ---\nPath,Created\n",
            "csv",
            "asc",
            true,
            true,
            "",
            "",
            0.0,
            false,
            "auto",
            10,
        )
        .unwrap_err();
        assert!(err.contains("section 'mft'") && err.contains("header row"), "{err}");
    }
}
