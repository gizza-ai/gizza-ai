//! gizza-ai/log-parser core — pure, no wafer/wasm-bindgen deps.
//!
//! Auto-detects the log format (JSON/NDJSON, logfmt, syslog, Apache/nginx
//! common + combined access logs) of raw log text and parses each line into a
//! normalized set of fields, then renders a filterable structured view as a
//! Markdown table, a JSON array, or CSV.
//!
//! A single unified **severity** (Trace < Debug < Info < Warn < Error) is
//! derived consistently across formats — from a JSON/logfmt `level`-style key,
//! a syslog PRI, or an access-log HTTP status (5xx → error, 4xx → warn) — so the
//! `level` filter (a minimum-severity threshold) works the same on every format.

use regex::Regex;
use std::fmt::Write as _;

const DEFAULT_LIMIT: u32 = 200;
pub const MAX_LIMIT: u32 = 5000;

/// The log format to parse with (or auto-detect).
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

/// How to render the parsed rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Output {
    Table,
    Json,
    Csv,
}

impl Output {
    pub fn parse(s: &str) -> Result<Output, String> {
        Ok(match s.trim().to_ascii_lowercase().as_str() {
            "" | "table" | "markdown" | "md" => Output::Table,
            "json" => Output::Json,
            "csv" => Output::Csv,
            other => return Err(format!("unknown output '{other}' (use table, json, or csv)")),
        })
    }
}

/// Unified severity, ordered least → most severe by discriminant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Trace = 0,
    Debug = 1,
    Info = 2,
    Warn = 3,
    Error = 4,
}

impl Severity {
    /// Map a free-text level word (case-insensitive) to a severity.
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

    /// Map a numeric syslog severity (0..=7) to a bucket.
    fn from_syslog(sev: u8) -> Severity {
        match sev {
            0..=3 => Severity::Error, // emerg, alert, crit, err
            4 => Severity::Warn,      // warning
            5 | 6 => Severity::Info,  // notice, info
            _ => Severity::Debug,     // debug (7) and anything higher
        }
    }

    /// Map an HTTP status code to a severity (5xx → error, 4xx → warn, else info).
    fn from_status(status: u16) -> Severity {
        match status {
            500..=599 => Severity::Error,
            400..=499 => Severity::Warn,
            _ => Severity::Info,
        }
    }
}

/// Minimum-severity filter selected by the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LevelFilter {
    All,
    Min(Severity),
}

impl LevelFilter {
    pub fn parse(s: &str) -> Result<LevelFilter, String> {
        Ok(match s.trim().to_ascii_lowercase().as_str() {
            "" | "all" => LevelFilter::All,
            "trace" => LevelFilter::Min(Severity::Trace),
            "debug" => LevelFilter::Min(Severity::Debug),
            "info" => LevelFilter::Min(Severity::Info),
            "warn" | "warning" => LevelFilter::Min(Severity::Warn),
            "error" => LevelFilter::Min(Severity::Error),
            other => {
                return Err(format!(
                    "unknown level '{other}' (use all, error, warn, info, debug, or trace)"
                ))
            }
        })
    }

    fn keeps(self, sev: Severity) -> bool {
        match self {
            LevelFilter::All => true,
            LevelFilter::Min(min) => sev >= min,
        }
    }
}

/// One parsed log line: ordered key/value fields + a normalized severity + the
/// original raw line (used for text/regex filtering).
#[derive(Debug, Clone)]
struct Entry {
    fields: Vec<(String, String)>,
    severity: Severity,
    raw: String,
}

// ---------------------------------------------------------------------------
// Compiled matchers (built once per parse call).
// ---------------------------------------------------------------------------

struct Matchers {
    combined: Regex,
    common: Regex,
    syslog_5424: Regex,
    syslog_3164: Regex,
    logfmt_pair: Regex,
}

impl Matchers {
    fn new() -> Matchers {
        // host ident authuser [date] "request" status size  [ "referer" "ua" ]
        let combined = Regex::new(
            r#"^(\S+) (\S+) (\S+) \[([^\]]+)\] "([^"]*)" (\d{3}|-) (\d+|-) "([^"]*)" "([^"]*)"\s*$"#,
        )
        .unwrap();
        let common =
            Regex::new(r#"^(\S+) (\S+) (\S+) \[([^\]]+)\] "([^"]*)" (\d{3}|-) (\d+|-)\s*$"#).unwrap();
        // RFC 5424: <PRI>VERSION TIMESTAMP HOST APP PROCID MSGID [SD/-] MSG
        let syslog_5424 = Regex::new(r"^<(\d{1,3})>(\d) (\S+) (\S+) (\S+) (\S+) (\S+) (.*)$").unwrap();
        // RFC 3164 (BSD): [<PRI>]Mmm dd HH:MM:SS HOST TAG[pid]: MSG
        let syslog_3164 = Regex::new(
            r"^(?:<(\d{1,3})>)?([A-Z][a-z]{2}\s+\d{1,2} \d{2}:\d{2}:\d{2}) (\S+) ([^:\[\s]+)(?:\[(\d+)\])?:?\s?(.*)$",
        )
        .unwrap();
        let logfmt_pair = Regex::new(r#"([A-Za-z0-9_.\-]+)=("[^"]*"|'[^']*'|[^\s]*)"#).unwrap();
        Matchers {
            combined,
            common,
            syslog_5424,
            syslog_3164,
            logfmt_pair,
        }
    }
}

// ---------------------------------------------------------------------------
// Per-line detection + parsing.
// ---------------------------------------------------------------------------

fn parse_json_line(line: &str) -> Option<Entry> {
    let t = line.trim();
    if !t.starts_with('{') {
        return None;
    }
    let v: serde_json::Value = serde_json::from_str(t).ok()?;
    let obj = v.as_object()?;
    let mut fields = Vec::new();
    let mut sev = None;
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
        fields.push((k.clone(), sval));
    }
    Some(Entry {
        fields,
        severity: sev.unwrap_or(Severity::Info),
        raw: line.to_string(),
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

fn parse_logfmt_line(m: &Matchers, line: &str) -> Option<Entry> {
    let t = line.trim();
    if t.starts_with('{') {
        return None;
    }
    let mut fields = Vec::new();
    let mut sev = None;
    let mut pairs = 0usize;
    let mut first_at_start = false;
    for (i, cap) in m.logfmt_pair.captures_iter(t).enumerate() {
        let whole = cap.get(0).unwrap();
        if i == 0 && whole.start() == 0 {
            first_at_start = true;
        }
        let key = cap[1].to_string();
        let val = strip_quotes(&cap[2]);
        let kl = key.to_ascii_lowercase();
        if sev.is_none() && matches!(kl.as_str(), "level" | "lvl" | "severity" | "loglevel") {
            sev = Severity::from_word(&val);
        }
        fields.push((key, val));
        pairs += 1;
    }
    // Require it to actually look like logfmt: at least two key=value pairs and
    // the very first token being one (so a prose line with one `x=y` isn't logfmt).
    if pairs < 2 || !first_at_start {
        return None;
    }
    Some(Entry {
        fields,
        severity: sev.unwrap_or(Severity::Info),
        raw: line.to_string(),
    })
}

fn parse_common_line(m: &Matchers, line: &str) -> Option<Entry> {
    let c = m.common.captures(line.trim_end())?;
    let status = c[6].parse::<u16>().ok();
    let fields = vec![
        ("ip".into(), c[1].to_string()),
        ("ident".into(), c[2].to_string()),
        ("user".into(), c[3].to_string()),
        ("time".into(), c[4].to_string()),
        ("request".into(), c[5].to_string()),
        ("status".into(), c[6].to_string()),
        ("size".into(), c[7].to_string()),
    ];
    Some(Entry {
        fields,
        severity: status.map(Severity::from_status).unwrap_or(Severity::Info),
        raw: line.to_string(),
    })
}

fn parse_combined_line(m: &Matchers, line: &str) -> Option<Entry> {
    let c = m.combined.captures(line.trim_end())?;
    let status = c[6].parse::<u16>().ok();
    let fields = vec![
        ("ip".into(), c[1].to_string()),
        ("ident".into(), c[2].to_string()),
        ("user".into(), c[3].to_string()),
        ("time".into(), c[4].to_string()),
        ("request".into(), c[5].to_string()),
        ("status".into(), c[6].to_string()),
        ("size".into(), c[7].to_string()),
        ("referer".into(), c[8].to_string()),
        ("user_agent".into(), c[9].to_string()),
    ];
    Some(Entry {
        fields,
        severity: status.map(Severity::from_status).unwrap_or(Severity::Info),
        raw: line.to_string(),
    })
}

fn parse_syslog_line(m: &Matchers, line: &str) -> Option<Entry> {
    let t = line.trim_end();
    if let Some(c) = m.syslog_5424.captures(t) {
        let pri: u16 = c[1].parse().ok()?;
        let sev = Severity::from_syslog((pri % 8) as u8);
        let fields = vec![
            ("timestamp".into(), c[3].to_string()),
            ("host".into(), c[4].to_string()),
            ("app".into(), c[5].to_string()),
            ("procid".into(), c[6].to_string()),
            ("msgid".into(), c[7].to_string()),
            ("message".into(), c[8].to_string()),
        ];
        return Some(Entry {
            fields,
            severity: sev,
            raw: line.to_string(),
        });
    }
    if let Some(c) = m.syslog_3164.captures(t) {
        let sev = c
            .get(1)
            .and_then(|g| g.as_str().parse::<u16>().ok())
            .map(|pri| Severity::from_syslog((pri % 8) as u8))
            .unwrap_or(Severity::Info);
        let fields = vec![
            ("timestamp".into(), c[2].to_string()),
            ("host".into(), c[3].to_string()),
            ("tag".into(), c[4].to_string()),
            ("pid".into(), c.get(5).map(|g| g.as_str()).unwrap_or("").to_string()),
            ("message".into(), c[6].to_string()),
        ];
        return Some(Entry {
            fields,
            severity: sev,
            raw: line.to_string(),
        });
    }
    None
}

/// Parse a single line with a KNOWN format; `None` if it doesn't match.
fn parse_line(m: &Matchers, fmt: Format, line: &str) -> Option<Entry> {
    match fmt {
        Format::Json => parse_json_line(line),
        Format::Logfmt => parse_logfmt_line(m, line),
        Format::Syslog => parse_syslog_line(m, line),
        Format::Common => parse_common_line(m, line),
        Format::Combined => parse_combined_line(m, line),
        Format::Auto => None,
    }
}

/// Classify a single line into its most specific format (or `None` if plain).
fn classify_line(m: &Matchers, line: &str) -> Option<Format> {
    if parse_json_line(line).is_some() {
        return Some(Format::Json);
    }
    if parse_combined_line(m, line).is_some() {
        return Some(Format::Combined);
    }
    if parse_common_line(m, line).is_some() {
        return Some(Format::Common);
    }
    if parse_syslog_line(m, line).is_some() {
        return Some(Format::Syslog);
    }
    if parse_logfmt_line(m, line).is_some() {
        return Some(Format::Logfmt);
    }
    None
}

/// Majority-vote auto-detection over a sample of the first non-empty lines.
fn detect_format(m: &Matchers, lines: &[&str]) -> Option<Format> {
    let mut counts = [0usize; 5]; // json, logfmt, syslog, common, combined
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
// Public entry point.
// ---------------------------------------------------------------------------

/// Parse raw `logs` and render the result.
///
/// - `format`: `auto` (default) | `json` | `logfmt` | `syslog` | `common` | `combined`.
/// - `output`: `table` (default, Markdown) | `json` | `csv`.
/// - `level`:  `all` (default) | `error` | `warn` | `info` | `debug` | `trace` — minimum severity.
/// - `filter`: keep only lines matching this (case-insensitive substring, or a regex if `use_regex`).
/// - `use_regex`: treat `filter` as a regular expression.
/// - `limit`: cap the number of rows rendered (0 → default 200; hard max 5000).
pub fn parse(
    logs: &str,
    format: &str,
    output: &str,
    level: &str,
    filter: &str,
    use_regex: bool,
    limit: u32,
) -> Result<String, String> {
    if logs.trim().is_empty() {
        return Err("input is empty — paste some log lines".into());
    }
    let fmt_req = Format::parse(format)?;
    let out = Output::parse(output)?;
    let lvl = LevelFilter::parse(level)?;
    let limit = if limit == 0 {
        DEFAULT_LIMIT
    } else {
        limit.clamp(1, MAX_LIMIT)
    };

    let m = Matchers::new();
    let lines: Vec<&str> = logs.lines().collect();

    let fmt = match fmt_req {
        Format::Auto => detect_format(&m, &lines).ok_or_else(|| {
            "could not auto-detect the log format — set format to json, logfmt, syslog, common, or combined"
                .to_string()
        })?,
        other => other,
    };

    // Parse every non-empty line; a line that doesn't match the chosen format
    // falls back to a single `message` field so nothing is silently dropped.
    let mut entries: Vec<Entry> = Vec::new();
    for line in &lines {
        if line.trim().is_empty() {
            continue;
        }
        let entry = parse_line(&m, fmt, line).unwrap_or_else(|| Entry {
            fields: vec![("message".into(), line.trim_end().to_string())],
            severity: Severity::Info,
            raw: line.to_string(),
        });
        entries.push(entry);
    }
    if entries.is_empty() {
        return Err("no log lines found".into());
    }
    let total = entries.len();

    // Optional text/regex filter on the raw line.
    let re = if !filter.is_empty() && use_regex {
        Some(Regex::new(filter).map_err(|e| format!("invalid regex '{filter}': {e}"))?)
    } else {
        None
    };
    let needle = filter.to_lowercase();
    let matches_filter = |e: &Entry| -> bool {
        if filter.is_empty() {
            return true;
        }
        match &re {
            Some(r) => r.is_match(&e.raw),
            None => e.raw.to_lowercase().contains(&needle),
        }
    };

    let shown: Vec<&Entry> = entries
        .iter()
        .filter(|e| lvl.keeps(e.severity) && matches_filter(e))
        .take(limit as usize)
        .collect();

    match out {
        Output::Table => Ok(render_table(fmt, total, &shown)),
        Output::Json => render_json(&shown),
        Output::Csv => Ok(render_csv(&shown)),
    }
}

// ---------------------------------------------------------------------------
// Rendering.
// ---------------------------------------------------------------------------

/// Union of field keys across `entries`, in first-seen order.
fn columns(entries: &[&Entry]) -> Vec<String> {
    let mut cols: Vec<String> = Vec::new();
    for e in entries {
        for (k, _) in &e.fields {
            if !cols.iter().any(|c| c == k) {
                cols.push(k.clone());
            }
        }
    }
    cols
}

fn field_get<'a>(e: &'a Entry, key: &str) -> &'a str {
    e.fields
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
        .unwrap_or("")
}

fn render_table(fmt: Format, total: usize, shown: &[&Entry]) -> String {
    let errors = shown.iter().filter(|e| e.severity == Severity::Error).count();
    let warns = shown.iter().filter(|e| e.severity == Severity::Warn).count();

    let mut caption = format!("{} · {} entries", fmt.label(), total);
    if shown.len() != total {
        let _ = write!(caption, " · {} shown", shown.len());
    }
    let _ = write!(caption, " · {errors} error · {warns} warn");

    if shown.is_empty() {
        return format!("{caption}\n\n(no rows match the current filter)");
    }

    let cols = columns(shown);
    let mut out = String::new();
    out.push_str(&caption);
    out.push_str("\n\n");
    out.push_str("| ");
    out.push_str(&cols.iter().map(|c| md_escape(c)).collect::<Vec<_>>().join(" | "));
    out.push_str(" |\n| ");
    out.push_str(&vec!["---"; cols.len()].join(" | "));
    out.push_str(" |\n");
    for e in shown {
        out.push_str("| ");
        out.push_str(
            &cols
                .iter()
                .map(|c| md_escape(field_get(e, c)))
                .collect::<Vec<_>>()
                .join(" | "),
        );
        out.push_str(" |\n");
    }
    out.pop(); // drop trailing newline
    out
}

fn md_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('|', "\\|").replace('\n', " ")
}

fn render_json(shown: &[&Entry]) -> Result<String, String> {
    let mut arr = Vec::with_capacity(shown.len());
    for e in shown {
        let mut obj = serde_json::Map::new();
        for (k, v) in &e.fields {
            obj.insert(k.clone(), serde_json::Value::String(v.clone()));
        }
        arr.push(serde_json::Value::Object(obj));
    }
    serde_json::to_string_pretty(&arr).map_err(|e| format!("failed to serialize JSON: {e}"))
}

fn render_csv(shown: &[&Entry]) -> String {
    let cols = columns(shown);
    let mut out = String::new();
    out.push_str(&cols.iter().map(|c| csv_escape(c)).collect::<Vec<_>>().join(","));
    out.push('\n');
    for e in shown {
        out.push_str(
            &cols
                .iter()
                .map(|c| csv_escape(field_get(e, c)))
                .collect::<Vec<_>>()
                .join(","),
        );
        out.push('\n');
    }
    out.pop();
    out
}

fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

// ---------------------------------------------------------------------------
// Tests.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const COMBINED: &str = r#"127.0.0.1 - alice [10/Oct/2000:13:55:36 -0700] "GET /index.html HTTP/1.0" 200 2326 "http://example.com/" "Mozilla/5.0"
10.0.0.2 - - [10/Oct/2000:13:55:37 -0700] "POST /api HTTP/1.1" 500 12 "-" "curl/7.0""#;

    const JSON_LOGS: &str = r#"{"time":"2024-01-01T00:00:00Z","level":"info","msg":"started"}
{"time":"2024-01-01T00:00:01Z","level":"error","msg":"boom"}"#;

    const LOGFMT: &str = r#"level=info msg="server up" port=8080
level=warn msg="slow query" ms=1200"#;

    const SYSLOG: &str = r#"<34>Oct 11 22:14:15 mymachine su: authentication failure
<13>Nov  1 09:00:00 host cron[123]: job ran"#;

    #[test]
    fn auto_detects_combined_and_maps_status_severity() {
        let out = parse(COMBINED, "auto", "table", "all", "", false, 0).unwrap();
        assert!(out.starts_with("combined · 2 entries"), "caption: {out}");
        assert!(out.contains("· 1 error · 0 warn"), "stats: {out}");
        assert!(out
            .contains("| ip | ident | user | time | request | status | size | referer | user_agent |"));
        assert!(out.contains("| 127.0.0.1 | - | alice |"));
    }

    #[test]
    fn auto_detects_json_lines() {
        let out = parse(JSON_LOGS, "auto", "table", "all", "", false, 0).unwrap();
        assert!(out.starts_with("json · 2 entries"), "{out}");
        assert!(out.contains("| time | level | msg |"));
        assert!(out.contains("boom"));
    }

    #[test]
    fn auto_detects_logfmt() {
        let out = parse(LOGFMT, "auto", "table", "all", "", false, 0).unwrap();
        assert!(out.starts_with("logfmt · 2 entries"), "{out}");
        assert!(out.contains("server up"));
        assert!(out.contains("slow query"));
    }

    #[test]
    fn auto_detects_syslog_and_pri_severity() {
        // <34> → severity 34%8 = 2 (crit) → Error bucket.
        let out = parse(SYSLOG, "syslog", "table", "all", "", false, 0).unwrap();
        assert!(out.contains("| timestamp | host | tag | pid | message |"));
        assert!(out.contains("authentication failure"));
        assert!(out.contains("· 1 error"), "{out}");
    }

    #[test]
    fn level_filter_keeps_errors_and_up() {
        let out = parse(JSON_LOGS, "auto", "table", "error", "", false, 0).unwrap();
        assert!(out.contains("· 1 shown"), "{out}");
        assert!(out.contains("boom"));
        assert!(!out.contains("started"));
    }

    #[test]
    fn text_filter_substring() {
        let out = parse(COMBINED, "auto", "table", "all", "post", false, 0).unwrap();
        assert!(out.contains("· 1 shown"), "{out}");
        assert!(out.contains("/api"));
        assert!(!out.contains("index.html"));
    }

    #[test]
    fn regex_filter() {
        let out = parse(COMBINED, "auto", "table", "all", r"\b50\d\b", true, 0).unwrap();
        assert!(out.contains("· 1 shown"), "{out}");
        assert!(out.contains("/api"));
    }

    #[test]
    fn output_json_is_valid_array() {
        let out = parse(JSON_LOGS, "auto", "json", "all", "", false, 0).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert!(v.is_array());
        assert_eq!(v.as_array().unwrap().len(), 2);
        assert_eq!(v[1]["msg"], "boom");
    }

    #[test]
    fn output_csv_header_and_values() {
        let out = parse(COMBINED, "combined", "csv", "all", "", false, 0).unwrap();
        let header = out.lines().next().unwrap();
        assert_eq!(
            header,
            "ip,ident,user,time,request,status,size,referer,user_agent"
        );
        assert!(out.contains("GET /index.html HTTP/1.0"));
    }

    #[test]
    fn limit_caps_rows() {
        let logs =
            "{\"level\":\"info\",\"i\":\"1\"}\n{\"level\":\"info\",\"i\":\"2\"}\n{\"level\":\"info\",\"i\":\"3\"}";
        let out = parse(logs, "auto", "table", "all", "", false, 2).unwrap();
        assert!(out.contains("· 2 shown"), "{out}");
    }

    #[test]
    fn empty_input_errors() {
        let err = parse("   \n  ", "auto", "table", "all", "", false, 0).unwrap_err();
        assert!(err.contains("empty"), "{err}");
    }

    #[test]
    fn undetectable_input_errors() {
        let err =
            parse("just some prose\nmore prose", "auto", "table", "all", "", false, 0).unwrap_err();
        assert!(err.contains("auto-detect"), "{err}");
    }

    #[test]
    fn bad_format_errors() {
        let err = parse("x", "xml", "table", "all", "", false, 0).unwrap_err();
        assert!(err.contains("unknown format"), "{err}");
    }

    #[test]
    fn bad_regex_errors() {
        let err = parse(COMBINED, "auto", "table", "all", "(", true, 0).unwrap_err();
        assert!(err.contains("invalid regex"), "{err}");
    }
}
