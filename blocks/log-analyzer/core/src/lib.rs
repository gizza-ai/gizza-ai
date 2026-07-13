//! gizza-ai/log-analyzer core — pure, no wafer/wasm-bindgen deps.
//!
//! Analyzes a chunk of raw log text and produces an aggregate SUMMARY rather
//! than a row-by-row view (that is the sibling `log-parser` tool's job). It:
//!   * auto-detects the format (JSON/NDJSON, logfmt, syslog, Apache/nginx
//!     common + combined access logs),
//!   * derives a unified severity per line,
//!   * counts entries by level,
//!   * groups the warning/error lines by a normalized message and ranks the
//!     most frequent ones ("top errors"),
//!   * extracts each line's timestamp to report the time span, and
//!   * buckets the entries into a volume timeline.
//!
//! Output is either a Markdown summary (default) or a structured JSON object.

use chrono::{DateTime, NaiveDateTime};
use regex::Regex;
use std::collections::HashMap;
use std::fmt::Write as _;

/// Hard cap on how many "top error" groups can be requested.
pub const MAX_TOP: u32 = 100;
const DEFAULT_TOP: u32 = 10;

// ---------------------------------------------------------------------------
// Enums (format / output / bucket) — parsed from user strings.
// ---------------------------------------------------------------------------

/// The log format to analyze with (or auto-detect).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Auto,
    Json,
    Logfmt,
    Syslog,
    Common,
    Combined,
}

impl Format {
    pub fn parse(s: &str) -> Result<Format, String> {
        Ok(match s.trim().to_ascii_lowercase().as_str() {
            "" | "auto" => Format::Auto,
            "json" | "ndjson" => Format::Json,
            "logfmt" => Format::Logfmt,
            "syslog" => Format::Syslog,
            "common" | "clf" => Format::Common,
            "combined" => Format::Combined,
            other => {
                return Err(format!(
                    "unknown format '{other}' (use auto, json, logfmt, syslog, common, or combined)"
                ))
            }
        })
    }

    fn label(self) -> &'static str {
        match self {
            Format::Auto => "auto",
            Format::Json => "json",
            Format::Logfmt => "logfmt",
            Format::Syslog => "syslog",
            Format::Common => "common",
            Format::Combined => "combined",
        }
    }
}

/// How to render the analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Output {
    Summary,
    Json,
}

impl Output {
    pub fn parse(s: &str) -> Result<Output, String> {
        Ok(match s.trim().to_ascii_lowercase().as_str() {
            "" | "summary" | "markdown" | "md" => Output::Summary,
            "json" => Output::Json,
            other => return Err(format!("unknown output '{other}' (use summary or json)")),
        })
    }
}

/// Timeline bucket granularity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bucket {
    Auto,
    Minute,
    Hour,
    Day,
}

impl Bucket {
    pub fn parse(s: &str) -> Result<Bucket, String> {
        Ok(match s.trim().to_ascii_lowercase().as_str() {
            "" | "auto" => Bucket::Auto,
            "minute" | "min" => Bucket::Minute,
            "hour" | "hr" => Bucket::Hour,
            "day" => Bucket::Day,
            other => {
                return Err(format!(
                    "unknown bucket '{other}' (use auto, minute, hour, or day)"
                ))
            }
        })
    }

    /// Seconds per bucket + the strftime pattern for the bucket label.
    fn spec(self) -> (i64, &'static str) {
        match self {
            Bucket::Minute => (60, "%Y-%m-%d %H:%M"),
            Bucket::Hour => (3600, "%Y-%m-%d %H:00"),
            Bucket::Day => (86400, "%Y-%m-%d"),
            Bucket::Auto => unreachable!("Auto is resolved before use"),
        }
    }

    /// Human-readable granularity for the summary heading.
    fn label(self) -> &'static str {
        match self {
            Bucket::Auto => "auto",
            Bucket::Minute => "per minute",
            Bucket::Hour => "per hour",
            Bucket::Day => "per day",
        }
    }

    /// Canonical key for JSON output.
    fn key(self) -> &'static str {
        match self {
            Bucket::Auto => "auto",
            Bucket::Minute => "minute",
            Bucket::Hour => "hour",
            Bucket::Day => "day",
        }
    }
}

// ---------------------------------------------------------------------------
// Unified severity.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    Trace = 0,
    Debug = 1,
    Info = 2,
    Warn = 3,
    Error = 4,
}

/// Least → most severe; used to iterate levels deterministically.
const SEV_ASC: [Severity; 5] = [
    Severity::Trace,
    Severity::Debug,
    Severity::Info,
    Severity::Warn,
    Severity::Error,
];

impl Severity {
    fn label(self) -> &'static str {
        match self {
            Severity::Trace => "trace",
            Severity::Debug => "debug",
            Severity::Info => "info",
            Severity::Warn => "warn",
            Severity::Error => "error",
        }
    }

    fn from_word(w: &str) -> Option<Severity> {
        Some(match w.trim().to_ascii_lowercase().as_str() {
            "trace" | "verbose" | "finest" => Severity::Trace,
            "debug" | "dbg" | "fine" => Severity::Debug,
            "info" | "information" | "informational" | "notice" | "log" => Severity::Info,
            "warn" | "warning" => Severity::Warn,
            "error" | "err" | "fatal" | "crit" | "critical" | "alert" | "emerg" | "emergency"
            | "panic" | "severe" => Severity::Error,
            _ => return None,
        })
    }

    fn from_syslog(sev: u8) -> Severity {
        match sev {
            0..=3 => Severity::Error,
            4 => Severity::Warn,
            5 | 6 => Severity::Info,
            _ => Severity::Debug,
        }
    }

    fn from_status(status: u16) -> Severity {
        match status {
            500..=599 => Severity::Error,
            400..=499 => Severity::Warn,
            _ => Severity::Info,
        }
    }
}

/// One analyzed line: its severity, an optional epoch-second timestamp, and a
/// short message used for the "top errors" grouping.
#[derive(Debug, Clone)]
struct Rec {
    severity: Severity,
    ts: Option<i64>,
    message: String,
}

// ---------------------------------------------------------------------------
// Compiled matchers.
// ---------------------------------------------------------------------------

struct Matchers {
    combined: Regex,
    common: Regex,
    syslog_5424: Regex,
    syslog_3164: Regex,
    logfmt_pair: Regex,
    digits: Regex,
    hexish: Regex,
}

impl Matchers {
    fn new() -> Matchers {
        let combined = Regex::new(
            r#"^(\S+) (\S+) (\S+) \[([^\]]+)\] "([^"]*)" (\d{3}|-) (\d+|-) "([^"]*)" "([^"]*)"\s*$"#,
        )
        .unwrap();
        let common =
            Regex::new(r#"^(\S+) (\S+) (\S+) \[([^\]]+)\] "([^"]*)" (\d{3}|-) (\d+|-)\s*$"#).unwrap();
        let syslog_5424 =
            Regex::new(r"^<(\d{1,3})>(\d) (\S+) (\S+) (\S+) (\S+) (\S+) (.*)$").unwrap();
        let syslog_3164 = Regex::new(
            r"^(?:<(\d{1,3})>)?([A-Z][a-z]{2}\s+\d{1,2} \d{2}:\d{2}:\d{2}) (\S+) ([^:\[\s]+)(?:\[(\d+)\])?:?\s?(.*)$",
        )
        .unwrap();
        let logfmt_pair = Regex::new(r#"([A-Za-z0-9_.\-]+)=("[^"]*"|'[^']*'|[^\s]*)"#).unwrap();
        // For normalizing messages when grouping "top errors".
        let hexish = Regex::new(r"\b[0-9a-fA-F]{8,}\b").unwrap();
        let digits = Regex::new(r"\d+").unwrap();
        Matchers {
            combined,
            common,
            syslog_5424,
            syslog_3164,
            logfmt_pair,
            digits,
            hexish,
        }
    }
}

// ---------------------------------------------------------------------------
// Timestamp parsing → epoch seconds (UTC).
// ---------------------------------------------------------------------------

/// Try a series of common timestamp shapes and return epoch seconds (UTC).
fn parse_ts(s: &str) -> Option<i64> {
    let t = s.trim();
    if t.is_empty() {
        return None;
    }

    // Pure numeric → unix epoch (seconds or milliseconds).
    if t.chars().all(|c| c.is_ascii_digit()) && t.len() >= 9 {
        if let Ok(n) = t.parse::<i64>() {
            // 13+ digits → milliseconds.
            return Some(if t.len() >= 13 { n / 1000 } else { n });
        }
    }

    // RFC 3339 / ISO 8601 with offset or Z.
    if let Ok(dt) = DateTime::parse_from_rfc3339(t) {
        return Some(dt.timestamp());
    }

    // Apache/nginx access-log date: 10/Oct/2000:13:55:36 -0700
    if let Ok(dt) = DateTime::parse_from_str(t, "%d/%b/%Y:%H:%M:%S %z") {
        return Some(dt.timestamp());
    }

    // Naive datetimes (assume UTC) in a few common shapes.
    for fmt in [
        "%Y-%m-%dT%H:%M:%S%.f",
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%d %H:%M:%S%.f",
        "%Y-%m-%d %H:%M:%S",
        "%Y/%m/%d %H:%M:%S",
    ] {
        if let Ok(ndt) = NaiveDateTime::parse_from_str(t, fmt) {
            return Some(ndt.and_utc().timestamp());
        }
    }

    // Syslog RFC 3164 date without a year: "Oct 11 22:14:15" — pin to a fixed
    // reference year so relative ordering / bucketing works.
    if let Ok(ndt) = NaiveDateTime::parse_from_str(&format!("1970 {t}"), "%Y %b %e %H:%M:%S") {
        return Some(ndt.and_utc().timestamp());
    }

    None
}

// ---------------------------------------------------------------------------
// Per-line extraction → Rec (severity, timestamp, message).
// ---------------------------------------------------------------------------

fn extract_json(line: &str) -> Option<Rec> {
    let t = line.trim();
    if !t.starts_with('{') {
        return None;
    }
    let v: serde_json::Value = serde_json::from_str(t).ok()?;
    let obj = v.as_object()?;
    let mut sev = None;
    let mut ts = None;
    let mut msg = None;
    for (k, val) in obj {
        let sval = match val {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Null => String::new(),
            other => other.to_string(),
        };
        let kl = k.to_ascii_lowercase();
        if sev.is_none()
            && matches!(
                kl.as_str(),
                "level" | "lvl" | "severity" | "loglevel" | "log.level" | "@level"
            )
        {
            sev = Severity::from_word(&sval);
        }
        if ts.is_none()
            && matches!(kl.as_str(), "time" | "ts" | "timestamp" | "@timestamp" | "date")
        {
            ts = parse_ts(&sval);
        }
        if msg.is_none() && matches!(kl.as_str(), "msg" | "message" | "error" | "event") {
            msg = Some(sval.clone());
        }
    }
    Some(Rec {
        severity: sev.unwrap_or(Severity::Info),
        ts,
        message: msg.unwrap_or_else(|| t.to_string()),
    })
}

fn strip_quotes(s: &str) -> String {
    let b = s.as_bytes();
    if b.len() >= 2
        && (b[0] == b'"' && b[b.len() - 1] == b'"' || b[0] == b'\'' && b[b.len() - 1] == b'\'')
    {
        s[1..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}

fn extract_logfmt(m: &Matchers, line: &str) -> Option<Rec> {
    let t = line.trim();
    if t.starts_with('{') {
        return None;
    }
    let mut sev = None;
    let mut ts = None;
    let mut msg = None;
    let mut pairs = 0usize;
    let mut first_at_start = false;
    for (i, cap) in m.logfmt_pair.captures_iter(t).enumerate() {
        let whole = cap.get(0).unwrap();
        if i == 0 && whole.start() == 0 {
            first_at_start = true;
        }
        let key = cap[1].to_ascii_lowercase();
        let val = strip_quotes(&cap[2]);
        if sev.is_none() && matches!(key.as_str(), "level" | "lvl" | "severity" | "loglevel") {
            sev = Severity::from_word(&val);
        }
        if ts.is_none() && matches!(key.as_str(), "time" | "ts" | "timestamp") {
            ts = parse_ts(&val);
        }
        if msg.is_none() && matches!(key.as_str(), "msg" | "message" | "error") {
            msg = Some(val.clone());
        }
        pairs += 1;
    }
    if pairs < 2 || !first_at_start {
        return None;
    }
    Some(Rec {
        severity: sev.unwrap_or(Severity::Info),
        ts,
        message: msg.unwrap_or_else(|| t.to_string()),
    })
}

fn extract_access(m: &Matchers, line: &str, combined: bool) -> Option<Rec> {
    let re = if combined { &m.combined } else { &m.common };
    let c = re.captures(line.trim_end())?;
    let status = c[6].parse::<u16>().ok();
    let ts = parse_ts(&c[4]);
    // message = the request line + status, so identical endpoints group together.
    let message = format!("{} {}", &c[5], &c[6]);
    Some(Rec {
        severity: status.map(Severity::from_status).unwrap_or(Severity::Info),
        ts,
        message,
    })
}

fn extract_syslog(m: &Matchers, line: &str) -> Option<Rec> {
    let t = line.trim_end();
    if let Some(c) = m.syslog_5424.captures(t) {
        let pri: u16 = c[1].parse().ok()?;
        return Some(Rec {
            severity: Severity::from_syslog((pri % 8) as u8),
            ts: parse_ts(&c[3]),
            message: c[8].to_string(),
        });
    }
    if let Some(c) = m.syslog_3164.captures(t) {
        let sev = c
            .get(1)
            .and_then(|g| g.as_str().parse::<u16>().ok())
            .map(|pri| Severity::from_syslog((pri % 8) as u8))
            .unwrap_or(Severity::Info);
        return Some(Rec {
            severity: sev,
            ts: parse_ts(&c[2]),
            message: c[6].to_string(),
        });
    }
    None
}

fn extract_line(m: &Matchers, fmt: Format, line: &str) -> Option<Rec> {
    match fmt {
        Format::Json => extract_json(line),
        Format::Logfmt => extract_logfmt(m, line),
        Format::Syslog => extract_syslog(m, line),
        Format::Common => extract_access(m, line, false),
        Format::Combined => extract_access(m, line, true),
        Format::Auto => None,
    }
}

fn classify_line(m: &Matchers, line: &str) -> Option<Format> {
    if extract_json(line).is_some() {
        return Some(Format::Json);
    }
    if extract_access(m, line, true).is_some() {
        return Some(Format::Combined);
    }
    if extract_access(m, line, false).is_some() {
        return Some(Format::Common);
    }
    if extract_syslog(m, line).is_some() {
        return Some(Format::Syslog);
    }
    if extract_logfmt(m, line).is_some() {
        return Some(Format::Logfmt);
    }
    None
}

fn detect_format(m: &Matchers, lines: &[&str]) -> Option<Format> {
    let mut counts = [0usize; 5];
    let idx = |f: Format| match f {
        Format::Json => 0,
        Format::Logfmt => 1,
        Format::Syslog => 2,
        Format::Common => 3,
        Format::Combined => 4,
        Format::Auto => unreachable!(),
    };
    let mut sampled = 0;
    for line in lines {
        if line.trim().is_empty() {
            continue;
        }
        if let Some(f) = classify_line(m, line) {
            counts[idx(f)] += 1;
        }
        sampled += 1;
        if sampled >= 40 {
            break;
        }
    }
    counts
        .iter()
        .enumerate()
        .max_by_key(|(_, &c)| c)
        .filter(|(_, &c)| c > 0)
        .map(|(i, _)| match i {
            0 => Format::Json,
            1 => Format::Logfmt,
            2 => Format::Syslog,
            3 => Format::Common,
            4 => Format::Combined,
            _ => unreachable!(),
        })
}

// ---------------------------------------------------------------------------
// Aggregation.
// ---------------------------------------------------------------------------

/// Mask variable parts of a message so near-identical errors group together:
/// long hex/uuid runs → `#`, then any remaining digit run → `#`.
fn normalize_message(m: &Matchers, msg: &str) -> String {
    let one_line: String = msg.split_whitespace().collect::<Vec<_>>().join(" ");
    let masked = m.hexish.replace_all(&one_line, "#");
    let masked = m.digits.replace_all(&masked, "#");
    let s = masked.trim();
    let truncated: String = s.chars().take(160).collect();
    if truncated.is_empty() {
        "(empty message)".to_string()
    } else {
        truncated
    }
}

struct Analysis {
    fmt: Format,
    total: usize,
    levels: [usize; 5], // indexed by Severity as usize
    top_errors: Vec<(String, Severity, usize)>,
    first_ts: Option<i64>,
    last_ts: Option<i64>,
    no_ts: usize,
    bucket: Bucket,
    timeline: Vec<(String, usize)>,
}

fn choose_bucket(span: i64) -> Bucket {
    if span <= 2 * 3600 {
        Bucket::Minute
    } else if span <= 2 * 86400 {
        Bucket::Hour
    } else {
        Bucket::Day
    }
}

fn analyze_recs(
    m: &Matchers,
    fmt: Format,
    recs: &[Rec],
    top: usize,
    bucket_req: Bucket,
) -> Analysis {
    let total = recs.len();
    let mut levels = [0usize; 5];
    let mut first_ts: Option<i64> = None;
    let mut last_ts: Option<i64> = None;
    let mut no_ts = 0usize;

    // Group warning+error messages for the "top errors" ranking. Preserve
    // first-seen order for stable ties and track the highest severity per group.
    let mut group_idx: HashMap<String, usize> = HashMap::new();
    let mut groups: Vec<(String, Severity, usize)> = Vec::new();

    for r in recs {
        levels[r.severity as usize] += 1;
        match r.ts {
            Some(ts) => {
                first_ts = Some(first_ts.map_or(ts, |c| c.min(ts)));
                last_ts = Some(last_ts.map_or(ts, |c| c.max(ts)));
            }
            None => no_ts += 1,
        }
        if r.severity >= Severity::Warn {
            let key = normalize_message(m, &r.message);
            match group_idx.get(&key) {
                Some(&i) => {
                    groups[i].2 += 1;
                    if r.severity > groups[i].1 {
                        groups[i].1 = r.severity;
                    }
                }
                None => {
                    group_idx.insert(key.clone(), groups.len());
                    groups.push((key, r.severity, 1));
                }
            }
        }
    }

    // Rank by count desc; a stable sort keeps first-seen order for ties.
    groups.sort_by(|a, b| b.2.cmp(&a.2));
    groups.truncate(top);

    // Timeline bucketing.
    let bucket = match bucket_req {
        Bucket::Auto => match (first_ts, last_ts) {
            (Some(a), Some(b)) => choose_bucket(b - a),
            _ => Bucket::Hour,
        },
        other => other,
    };
    let (step, pat) = bucket.spec();
    let mut tl_idx: HashMap<i64, usize> = HashMap::new();
    let mut tl: Vec<(i64, usize)> = Vec::new();
    for r in recs {
        if let Some(ts) = r.ts {
            let key = ts - ts.rem_euclid(step);
            match tl_idx.get(&key) {
                Some(&i) => tl[i].1 += 1,
                None => {
                    tl_idx.insert(key, tl.len());
                    tl.push((key, 1));
                }
            }
        }
    }
    tl.sort_by_key(|(k, _)| *k);
    let timeline: Vec<(String, usize)> = tl
        .into_iter()
        .filter_map(|(k, n)| {
            DateTime::from_timestamp(k, 0).map(|dt| (dt.format(pat).to_string(), n))
        })
        .collect();

    Analysis {
        fmt,
        total,
        levels,
        top_errors: groups,
        first_ts,
        last_ts,
        no_ts,
        bucket,
        timeline,
    }
}

// ---------------------------------------------------------------------------
// Rendering.
// ---------------------------------------------------------------------------

fn fmt_epoch(ts: i64) -> String {
    DateTime::from_timestamp(ts, 0)
        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_else(|| ts.to_string())
}

fn md_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('|', "\\|").replace('\n', " ")
}

fn render_summary(a: &Analysis) -> String {
    let mut out = String::new();

    // Caption line.
    let _ = write!(out, "{} · {} entries", a.fmt.label(), a.total);
    match (a.first_ts, a.last_ts) {
        (Some(f), Some(l)) => {
            let _ = write!(out, " · {} → {}", fmt_epoch(f), fmt_epoch(l));
        }
        _ => out.push_str(" · no timestamps"),
    }

    // Levels (most severe first, skip levels with no entries).
    out.push_str("\n\n## Levels\n\n");
    out.push_str("| level | count | share |\n| --- | --- | --- |\n");
    for sev in SEV_ASC.iter().rev().copied() {
        let n = a.levels[sev as usize];
        if n == 0 {
            continue;
        }
        let pct = 100.0 * n as f64 / a.total as f64;
        let _ = writeln!(out, "| {} | {} | {:.1}% |", sev.label(), n, pct);
    }

    // Top errors.
    out.push_str("\n## Top errors\n\n");
    if a.top_errors.is_empty() {
        out.push_str("No warnings or errors found.\n");
    } else {
        out.push_str("| count | level | message |\n| --- | --- | --- |\n");
        for (msg, sev, n) in &a.top_errors {
            let _ = writeln!(out, "| {} | {} | {} |", n, sev.label(), md_escape(msg));
        }
    }

    // Volume timeline.
    let _ = write!(out, "\n## Volume timeline ({})\n\n", a.bucket.label());
    if a.timeline.is_empty() {
        out.push_str("No timestamps found — timeline unavailable.\n");
    } else {
        let peak = a.timeline.iter().map(|(_, n)| *n).max().unwrap_or(1).max(1);
        out.push_str("| time bucket | count | |\n| --- | --- | --- |\n");
        for (label, n) in &a.timeline {
            let bars = ((*n as f64 / peak as f64) * 20.0).round() as usize;
            let bar = "▇".repeat(bars.max(1));
            let _ = writeln!(out, "| {} | {} | {} |", label, n, bar);
        }
        if a.no_ts > 0 {
            let _ = writeln!(out, "| (no timestamp) | {} | |", a.no_ts);
        }
    }

    out.pop(); // trailing newline
    out
}

fn render_json(a: &Analysis) -> Result<String, String> {
    let mut levels = serde_json::Map::new();
    for sev in SEV_ASC {
        levels.insert(
            sev.label().to_string(),
            serde_json::Value::from(a.levels[sev as usize]),
        );
    }

    let top: Vec<serde_json::Value> = a
        .top_errors
        .iter()
        .map(|(msg, sev, n)| serde_json::json!({ "count": n, "level": sev.label(), "message": msg }))
        .collect();

    let points: Vec<serde_json::Value> = a
        .timeline
        .iter()
        .map(|(label, n)| serde_json::json!({ "time": label, "count": n }))
        .collect();

    let obj = serde_json::json!({
        "format": a.fmt.label(),
        "total": a.total,
        "levels": levels,
        "first_timestamp": a.first_ts.map(fmt_epoch),
        "last_timestamp": a.last_ts.map(fmt_epoch),
        "entries_without_timestamp": a.no_ts,
        "top_errors": top,
        "timeline": { "bucket": a.bucket.key(), "points": points },
    });

    serde_json::to_string_pretty(&obj).map_err(|e| format!("failed to serialize JSON: {e}"))
}

// ---------------------------------------------------------------------------
// Public entry point.
// ---------------------------------------------------------------------------

/// Analyze raw `logs` and render a summary (default) or JSON.
///
/// - `format`: `auto` (default) | `json` | `logfmt` | `syslog` | `common` | `combined`.
/// - `output`: `summary` (default, Markdown) | `json`.
/// - `top`: how many top error/warning groups to list (0 → default 10; max 100).
/// - `bucket`: timeline granularity — `auto` (default) | `minute` | `hour` | `day`.
pub fn analyze(
    logs: &str,
    format: &str,
    output: &str,
    top: u32,
    bucket: &str,
) -> Result<String, String> {
    if logs.trim().is_empty() {
        return Err("input is empty — paste some log lines".into());
    }
    let fmt_req = Format::parse(format)?;
    let out = Output::parse(output)?;
    let bucket_req = Bucket::parse(bucket)?;
    let top = if top == 0 {
        DEFAULT_TOP
    } else {
        top.clamp(1, MAX_TOP)
    } as usize;

    let m = Matchers::new();
    let lines: Vec<&str> = logs.lines().collect();

    let fmt = match fmt_req {
        Format::Auto => detect_format(&m, &lines).ok_or_else(|| {
            "could not auto-detect the log format — set format to json, logfmt, syslog, common, or combined"
                .to_string()
        })?,
        other => other,
    };

    // Lines that don't match the chosen format fall back to a plain message at
    // info severity, so counts stay honest and nothing is dropped.
    let mut recs: Vec<Rec> = Vec::new();
    for line in &lines {
        if line.trim().is_empty() {
            continue;
        }
        let rec = extract_line(&m, fmt, line).unwrap_or_else(|| Rec {
            severity: Severity::Info,
            ts: None,
            message: line.trim().to_string(),
        });
        recs.push(rec);
    }
    if recs.is_empty() {
        return Err("no log lines found".into());
    }

    let analysis = analyze_recs(&m, fmt, &recs, top, bucket_req);
    match out {
        Output::Summary => Ok(render_summary(&analysis)),
        Output::Json => render_json(&analysis),
    }
}

// ---------------------------------------------------------------------------
// Tests.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const JSON_LOGS: &str = r#"{"ts":"2024-01-01T00:00:00Z","level":"info","msg":"server started"}
{"ts":"2024-01-01T00:20:00Z","level":"warn","msg":"high latency 900ms"}
{"ts":"2024-01-01T00:40:00Z","level":"error","msg":"db timeout attempt 3"}
{"ts":"2024-01-01T00:41:00Z","level":"error","msg":"db timeout attempt 5"}"#;

    const COMBINED: &str = r#"127.0.0.1 - - [10/Oct/2000:13:55:36 -0700] "GET /index.html HTTP/1.0" 200 2326 "-" "Mozilla/5.0"
10.0.0.5 - - [10/Oct/2000:13:55:37 -0700] "GET /missing HTTP/1.1" 404 512 "-" "curl/7.0"
10.0.0.9 - - [10/Oct/2000:13:55:38 -0700] "POST /api HTTP/1.1" 500 12 "-" "Go-http-client/1.1""#;

    #[test]
    fn json_summary_levels_and_top_errors() {
        let out = analyze(JSON_LOGS, "auto", "summary", 0, "auto").unwrap();
        assert!(
            out.starts_with("json · 4 entries · 2024-01-01 00:00:00 → 2024-01-01 00:41:00"),
            "{out}"
        );
        assert!(out.contains("| error | 2 | 50.0% |"), "{out}");
        assert!(out.contains("| warn | 1 | 25.0% |"), "{out}");
        assert!(out.contains("| info | 1 | 25.0% |"), "{out}");
        // The two "db timeout attempt N" errors group into one (digits masked).
        assert!(out.contains("| 2 | error | db timeout attempt # |"), "{out}");
        assert!(out.contains("## Volume timeline (per minute)"), "{out}");
    }

    #[test]
    fn json_output_is_valid_object() {
        let out = analyze(JSON_LOGS, "auto", "json", 5, "hour").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["format"], "json");
        assert_eq!(v["total"], 4);
        assert_eq!(v["levels"]["error"], 2);
        assert_eq!(v["levels"]["info"], 1);
        assert_eq!(v["top_errors"][0]["count"], 2);
        assert_eq!(v["top_errors"][0]["message"], "db timeout attempt #");
        assert_eq!(v["timeline"]["bucket"], "hour");
        assert_eq!(v["timeline"]["points"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn combined_access_log_status_severity() {
        let out = analyze(COMBINED, "auto", "summary", 0, "auto").unwrap();
        assert!(out.starts_with("combined · 3 entries"), "{out}");
        assert!(out.contains("| error | 1 |"), "{out}");
        assert!(out.contains("| warn | 1 |"), "{out}");
        // The 500 request line becomes a top error; digits (HTTP version + status)
        // are masked by the "top errors" grouping normalizer.
        assert!(out.contains("| 1 | error | POST /api HTTP/#.# # |"), "{out}");
    }

    #[test]
    fn no_errors_reports_none() {
        let logs = "{\"level\":\"info\",\"msg\":\"a\"}\n{\"level\":\"info\",\"msg\":\"b\"}";
        let out = analyze(logs, "auto", "summary", 0, "auto").unwrap();
        assert!(out.contains("No warnings or errors found."), "{out}");
    }

    #[test]
    fn top_limit_caps_groups() {
        let out = analyze(JSON_LOGS, "json", "json", 1, "auto").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["top_errors"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn logs_without_timestamp_note_no_timeline() {
        let logs = "level=error msg=\"boom\" code=1\nlevel=warn msg=\"careful\" code=2";
        let out = analyze(logs, "auto", "summary", 0, "auto").unwrap();
        assert!(out.contains("· no timestamps"), "{out}");
        assert!(out.contains("timeline unavailable"), "{out}");
    }

    #[test]
    fn empty_input_errors() {
        let err = analyze("   \n ", "auto", "summary", 0, "auto").unwrap_err();
        assert!(err.contains("empty"), "{err}");
    }

    #[test]
    fn bad_format_errors() {
        let err = analyze("x", "xml", "summary", 0, "auto").unwrap_err();
        assert!(err.contains("unknown format"), "{err}");
    }

    #[test]
    fn bad_output_errors() {
        let err = analyze(JSON_LOGS, "auto", "table", 0, "auto").unwrap_err();
        assert!(err.contains("unknown output"), "{err}");
    }

    #[test]
    fn bad_bucket_errors() {
        let err = analyze(JSON_LOGS, "auto", "summary", 0, "week").unwrap_err();
        assert!(err.contains("unknown bucket"), "{err}");
    }

    #[test]
    fn undetectable_input_errors() {
        let err = analyze("just prose\nmore prose", "auto", "summary", 0, "auto").unwrap_err();
        assert!(err.contains("auto-detect"), "{err}");
    }
}
