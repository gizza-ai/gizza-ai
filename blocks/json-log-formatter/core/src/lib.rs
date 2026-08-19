//! json-log-formatter core — pure compute, shared by the chat skill block and the
//! web page. No wafer/wasm-bindgen deps, no I/O, no clock: the same input always
//! renders the same bytes.
//!
//! Takes NDJSON / JSON Lines (one JSON object per line) and renders it as a
//! readable log view:
//!
//! ```text
//! [2024-01-01T00:00:00Z] INFO  server started port=8080
//! [2024-01-01T00:00:09Z] ERROR db timeout attempt=3 req.method=GET
//! ```
//!
//! Blank lines and `#`/`//` comment lines are skipped. Nested objects flatten to
//! dotted keys (`req.method`, `items.0.id`). The `time`/`level`/`message` keys are
//! auto-detected across the usual aliases, or named explicitly. A minimum-severity
//! filter understands level words, bunyan-style numbers (10/20/30/40/50/60) and
//! syslog priorities (7/6/5/4/3/2). Output is a pretty log view, a Markdown table,
//! a JSON array, or CSV.

use serde_json::{Map, Value};
use std::fmt::Write as _;

/// Hard cap on rendered records — the schema's `limit` maximum references this.
pub const MAX_LIMIT: u32 = 5000;
/// Applied when `limit` is 0 / blank (i.e. "not supplied").
pub const DEFAULT_LIMIT: u32 = 200;

/// Key names checked, in order, when `time_field` is blank.
const TIME_ALIASES: &[&str] = &[
    "time",
    "ts",
    "timestamp",
    "@timestamp",
    "datetime",
    "date",
    "t",
];
/// Key names checked, in order, when `level_field` is blank.
const LEVEL_ALIASES: &[&str] = &[
    "level",
    "lvl",
    "severity",
    "levelname",
    "loglevel",
    "log_level",
    "@level",
];
/// Key names checked, in order, when `message_field` is blank.
const MESSAGE_ALIASES: &[&str] = &["message", "msg", "text", "body", "event", "log"];

// ------------------------------------------------------------------ severity

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Severity {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
    Fatal,
}

impl Severity {
    fn name(self) -> &'static str {
        match self {
            Severity::Trace => "TRACE",
            Severity::Debug => "DEBUG",
            Severity::Info => "INFO",
            Severity::Warn => "WARN",
            Severity::Error => "ERROR",
            Severity::Fatal => "FATAL",
        }
    }

    /// A level *word*. Unknown words are not an error — a custom vocabulary still
    /// renders verbatim, it just sorts as `info` for the minimum-severity filter.
    fn from_word(w: &str) -> Option<Severity> {
        Some(match w.trim().to_ascii_lowercase().as_str() {
            "trace" | "verbose" | "trce" => Severity::Trace,
            "debug" | "dbg" | "dbug" | "fine" => Severity::Debug,
            "info" | "information" | "informational" | "notice" | "log" => Severity::Info,
            "warn" | "warning" | "wrn" => Severity::Warn,
            "error" | "err" | "eror" | "severe" => Severity::Error,
            "fatal" | "crit" | "critical" | "panic" | "alert" | "emerg" | "emergency" => {
                Severity::Fatal
            }
            _ => return None,
        })
    }

    /// A numeric level. Two conventions are in the wild and they don't overlap:
    /// bunyan/pino use 10/20/30/40/50/60 (trace…fatal, higher = worse), syslog
    /// uses 0-7 (higher = *less* severe). Anything below 10 is read as syslog.
    fn from_number(n: f64) -> Severity {
        if n < 10.0 {
            // syslog: 7=debug 6=info 5=notice 4=warning 3=error 2=crit 1=alert 0=emerg
            match n.round() as i64 {
                i if i >= 7 => Severity::Debug,
                6 | 5 => Severity::Info,
                4 => Severity::Warn,
                3 => Severity::Error,
                _ => Severity::Fatal,
            }
        } else if n < 20.0 {
            Severity::Trace
        } else if n < 30.0 {
            Severity::Debug
        } else if n < 40.0 {
            Severity::Info
        } else if n < 50.0 {
            Severity::Warn
        } else if n < 60.0 {
            Severity::Error
        } else {
            Severity::Fatal
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum LevelFilter {
    All,
    Min(Severity),
}

impl LevelFilter {
    fn parse(s: &str) -> Result<LevelFilter, String> {
        Ok(match s.trim().to_ascii_lowercase().as_str() {
            "" | "all" => LevelFilter::All,
            "trace" => LevelFilter::Min(Severity::Trace),
            "debug" => LevelFilter::Min(Severity::Debug),
            "info" => LevelFilter::Min(Severity::Info),
            "warn" | "warning" => LevelFilter::Min(Severity::Warn),
            "error" => LevelFilter::Min(Severity::Error),
            "fatal" => LevelFilter::Min(Severity::Fatal),
            other => {
                return Err(format!(
                "unknown level '{other}' — use all, trace, debug, info, warn, error, or fatal"
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

// ------------------------------------------------------------------ enums

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum MatchMode {
    Contains,
    Exact,
}

impl MatchMode {
    fn parse(s: &str) -> Result<MatchMode, String> {
        Ok(match s.trim().to_ascii_lowercase().as_str() {
            "" | "contains" => MatchMode::Contains,
            "exact" => MatchMode::Exact,
            other => return Err(format!("unknown match '{other}' — use contains or exact")),
        })
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum OnInvalid {
    Skip,
    Keep,
    Error,
}

impl OnInvalid {
    fn parse(s: &str) -> Result<OnInvalid, String> {
        Ok(match s.trim().to_ascii_lowercase().as_str() {
            "" | "skip" => OnInvalid::Skip,
            "keep" => OnInvalid::Keep,
            "error" => OnInvalid::Error,
            other => {
                return Err(format!(
                    "unknown on_invalid '{other}' — use skip, keep, or error"
                ))
            }
        })
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Output {
    Pretty,
    Table,
    Json,
    Csv,
}

impl Output {
    fn parse(s: &str) -> Result<Output, String> {
        Ok(match s.trim().to_ascii_lowercase().as_str() {
            "" | "pretty" => Output::Pretty,
            "table" => Output::Table,
            "json" => Output::Json,
            "csv" => Output::Csv,
            other => {
                return Err(format!(
                    "unknown output '{other}' — use pretty, table, json, or csv"
                ))
            }
        })
    }
}

// ------------------------------------------------------------------ values

/// How a JSON value reads in a text cell: strings unquoted, everything else as
/// compact JSON (so `null` stays visible and objects don't explode a column).
fn display_value(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

fn display_opt(v: Option<&Value>) -> String {
    v.map(display_value).unwrap_or_default()
}

/// Resolve a dotted path against a record. Numeric segments index into arrays.
/// A literal key containing dots wins over the dotted walk, so a record that
/// really has a `"req.method"` key is still addressable.
fn path_get<'a>(root: &'a Value, path: &str) -> Option<&'a Value> {
    if let Value::Object(m) = root {
        if let Some(v) = m.get(path) {
            return Some(v);
        }
    }
    let mut cur = root;
    for seg in path.split('.') {
        cur = match cur {
            Value::Object(m) => m.get(seg)?,
            Value::Array(a) => a.get(seg.parse::<usize>().ok()?)?,
            _ => return None,
        };
    }
    Some(cur)
}

/// Expand a record into ordered `(dotted key, value)` pairs. With `flatten`,
/// nested objects/arrays recurse (`req.method`, `items.0.id`); empty containers
/// stop the recursion so the key never vanishes. Without it, a nested value stays
/// whole and renders as compact JSON.
fn flatten_record(record: &Value, flatten: bool) -> Vec<(String, Value)> {
    fn walk(key: String, v: &Value, flatten: bool, out: &mut Vec<(String, Value)>) {
        match v {
            Value::Object(m) if flatten && !m.is_empty() => {
                for (k, child) in m {
                    walk(format!("{key}.{k}"), child, flatten, out);
                }
            }
            Value::Array(a) if flatten && !a.is_empty() => {
                for (i, child) in a.iter().enumerate() {
                    walk(format!("{key}.{i}"), child, flatten, out);
                }
            }
            other => out.push((key, other.clone())),
        }
    }
    let mut out = Vec::new();
    if let Value::Object(m) = record {
        for (k, v) in m {
            walk(k.clone(), v, flatten, &mut out);
        }
    }
    out
}

// ------------------------------------------------------------------ timestamps

/// Civil date from a days-since-1970 count (Howard Hinnant's `civil_from_days`).
fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn iso_from_epoch_ms(ms: i64) -> String {
    let days = ms.div_euclid(86_400_000);
    let rem = ms.rem_euclid(86_400_000);
    let (y, mo, d) = civil_from_days(days);
    let (secs, milli) = (rem / 1000, rem % 1000);
    let (h, mi, s) = (secs / 3600, (secs % 3600) / 60, secs % 60);
    if milli != 0 {
        format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}.{milli:03}Z")
    } else {
        format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
    }
}

/// Render a timestamp value. Strings pass through untouched (they are already
/// whatever the writer chose); numbers are epoch seconds, or epoch milliseconds
/// once they are too large to be seconds (pino's default), and become ISO 8601 UTC.
fn render_time(v: &Value) -> String {
    match v {
        Value::Number(n) => match n.as_f64() {
            Some(f) if f.is_finite() => {
                let ms = if f.abs() >= 1e11 { f } else { f * 1000.0 };
                iso_from_epoch_ms(ms.round() as i64)
            }
            _ => display_value(v),
        },
        other => display_value(other),
    }
}

// ------------------------------------------------------------------ records

struct Record {
    /// The parsed object (for a kept invalid line: `{"message": "<raw line>"}`).
    value: Value,
    /// What a blank-`field` `contains` search runs against.
    search: String,
    flat: Vec<(String, Value)>,
    /// Dotted paths consumed by the time/level/message columns — excluded from
    /// the pretty view's trailing `key=value` pairs.
    consumed: Vec<String>,
    time: Option<String>,
    level_text: Option<String>,
    message: String,
    severity: Severity,
}

/// Find the first alias present at the top level of `record`, or use `explicit`
/// (a dotted path) when the caller named the key.
fn pick_field<'a>(
    record: &'a Value,
    explicit: &str,
    aliases: &[&str],
) -> Option<(String, &'a Value)> {
    let explicit = explicit.trim();
    if !explicit.is_empty() {
        return path_get(record, explicit).map(|v| (explicit.to_string(), v));
    }
    let m = record.as_object()?;
    aliases
        .iter()
        .find_map(|a| m.get(*a).map(|v| ((*a).to_string(), v)))
}

#[allow(clippy::too_many_arguments)]
fn build_record(
    value: Value,
    search: String,
    flatten: bool,
    level_field: &str,
    time_field: &str,
    message_field: &str,
) -> Record {
    let mut consumed = Vec::new();

    let time = pick_field(&value, time_field, TIME_ALIASES).map(|(k, v)| {
        consumed.push(k);
        render_time(v)
    });

    let mut severity = Severity::Info;
    let level_text = pick_field(&value, level_field, LEVEL_ALIASES).map(|(k, v)| {
        consumed.push(k);
        match v {
            Value::Number(n) => {
                severity = Severity::from_number(n.as_f64().unwrap_or(30.0));
                severity.name().to_string()
            }
            Value::String(s) => {
                // A stringified number ("30") is still a numeric level.
                severity = match Severity::from_word(s) {
                    Some(sev) => sev,
                    None => match s.trim().parse::<f64>() {
                        Ok(n) => Severity::from_number(n),
                        Err(_) => Severity::Info,
                    },
                };
                s.trim().to_uppercase()
            }
            other => display_value(other),
        }
    });

    let message = pick_field(&value, message_field, MESSAGE_ALIASES)
        .map(|(k, v)| {
            consumed.push(k);
            display_value(v)
        })
        .unwrap_or_default();

    Record {
        flat: flatten_record(&value, flatten),
        value,
        search,
        consumed,
        time,
        level_text,
        message,
        severity,
    }
}

// ------------------------------------------------------------------ entry point

/// Format NDJSON log lines. See the module docs for the shape of each output.
///
/// * `input` — the raw NDJSON/JSONL text; blank and `#`/`//` comment lines are skipped.
/// * `level` — minimum severity: `all` (blank) | trace | debug | info | warn | error | fatal.
/// * `field` / `filter` / `match_mode` — the record filter (blank `field` searches
///   the whole record; `contains` is case-insensitive, `exact` compares the
///   stringified value).
/// * `fields` — comma-separated dotted paths to keep, in order (blank = every key).
/// * `level_field` / `time_field` / `message_field` — explicit key names (blank = auto-detect).
/// * `flatten` — expand nested objects/arrays into dotted keys.
/// * `on_invalid` — `skip` | `keep` | `error` for lines that aren't a JSON object.
/// * `limit` — max records rendered, 1..=`MAX_LIMIT`; 0 means `DEFAULT_LIMIT`.
/// * `output` — `pretty` (blank) | `table` | `json` | `csv`.
#[allow(clippy::too_many_arguments)]
pub fn format_logs(
    input: &str,
    level: &str,
    field: &str,
    filter: &str,
    match_mode: &str,
    fields: &str,
    level_field: &str,
    time_field: &str,
    message_field: &str,
    flatten: bool,
    on_invalid: &str,
    limit: u32,
    output: &str,
) -> Result<String, String> {
    let level = LevelFilter::parse(level)?;
    let match_mode = MatchMode::parse(match_mode)?;
    let on_invalid = OnInvalid::parse(on_invalid)?;
    let output = Output::parse(output)?;
    let limit = if limit == 0 { DEFAULT_LIMIT } else { limit }.clamp(1, MAX_LIMIT) as usize;

    let selected: Vec<String> = fields
        .split(',')
        .map(|f| f.trim().to_string())
        .filter(|f| !f.is_empty())
        .collect();
    let field = field.trim();
    let filter = filter.trim();

    if input.trim().is_empty() {
        return Err("no input — paste NDJSON / JSON Lines, one JSON object per line".into());
    }

    // --- parse -------------------------------------------------------------
    let mut records: Vec<Record> = Vec::new();
    let mut invalid = 0usize;
    for (i, raw) in input.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
            continue;
        }
        let parsed = serde_json::from_str::<Value>(line);
        let obj = match parsed {
            Ok(v @ Value::Object(_)) => Some(v),
            Ok(_) => None,
            Err(_) => None,
        };
        match obj {
            Some(v) => {
                let search = v.to_string();
                records.push(build_record(
                    v,
                    search,
                    flatten,
                    level_field,
                    time_field,
                    message_field,
                ));
            }
            None => {
                invalid += 1;
                match on_invalid {
                    OnInvalid::Skip => {}
                    OnInvalid::Error => {
                        return Err(format!(
                            "line {} is not a JSON object: {line} — set on_invalid to skip or keep to tolerate it",
                            i + 1
                        ))
                    }
                    OnInvalid::Keep => {
                        let mut m = Map::new();
                        m.insert("message".into(), Value::String(line.to_string()));
                        records.push(build_record(
                            Value::Object(m),
                            line.to_string(),
                            flatten,
                            level_field,
                            time_field,
                            message_field,
                        ));
                    }
                }
            }
        }
    }

    if records.is_empty() && invalid == 0 {
        return Err("no JSON records found — every line was blank or a comment".into());
    }

    // --- filter ------------------------------------------------------------
    let total = records.len();
    let kept: Vec<&Record> = records
        .iter()
        .filter(|r| level.keeps(r.severity))
        .filter(|r| matches_filter(r, field, filter, match_mode))
        .collect();
    let matched = kept.len();
    let shown: Vec<&Record> = kept.into_iter().take(limit).collect();

    let caption = caption_for(total, matched, shown.len(), invalid, on_invalid);
    let columns = columns_for(&shown, &selected);

    Ok(match output {
        Output::Pretty => render_pretty(&caption, &shown, &selected),
        Output::Table => render_table(&caption, &shown, &columns),
        Output::Json => render_json(&shown, &columns, selected.is_empty()),
        Output::Csv => render_csv(&shown, &columns),
    })
}

fn matches_filter(r: &Record, field: &str, filter: &str, mode: MatchMode) -> bool {
    if filter.is_empty() {
        return true;
    }
    if field.is_empty() {
        return match mode {
            // The whole serialized record, so nested values match too.
            MatchMode::Contains => r.search.to_lowercase().contains(&filter.to_lowercase()),
            // Any single field whose value is exactly this.
            MatchMode::Exact => r.flat.iter().any(|(_, v)| display_value(v) == filter),
        };
    }
    match path_get(&r.value, field) {
        None => false,
        Some(v) => {
            let s = display_value(v);
            match mode {
                MatchMode::Contains => s.to_lowercase().contains(&filter.to_lowercase()),
                MatchMode::Exact => s == filter,
            }
        }
    }
}

fn caption_for(
    total: usize,
    matched: usize,
    shown: usize,
    invalid: usize,
    on_invalid: OnInvalid,
) -> String {
    let mut c = format!("{total} record{}", if total == 1 { "" } else { "s" });
    if matched != total || shown != total {
        let _ = write!(c, " · {shown} shown");
    }
    if invalid > 0 {
        let verb = if on_invalid == OnInvalid::Keep {
            "kept"
        } else {
            "skipped"
        };
        let _ = write!(
            c,
            " · {invalid} invalid line{} {verb}",
            if invalid == 1 { "" } else { "s" }
        );
    }
    c
}

/// Table/CSV/JSON columns: the explicit `fields` selection, else the union of
/// every key seen, in first-seen order (so heterogeneous records still line up).
fn columns_for(shown: &[&Record], selected: &[String]) -> Vec<String> {
    if !selected.is_empty() {
        return selected.to_vec();
    }
    let mut cols: Vec<String> = Vec::new();
    for r in shown {
        for (k, _) in &r.flat {
            if !cols.iter().any(|c| c == k) {
                cols.push(k.clone());
            }
        }
    }
    cols
}

/// The trailing `key=value` pairs of a pretty line: the explicit selection (missing
/// paths render empty, as documented) or everything the header didn't already show.
fn extras_for<'a>(r: &'a Record, selected: &[String]) -> Vec<(String, String)> {
    if !selected.is_empty() {
        return selected
            .iter()
            .map(|p| (p.clone(), display_opt(path_get(&r.value, p))))
            .collect();
    }
    r.flat
        .iter()
        .filter(|(k, _)| {
            !r.consumed
                .iter()
                .any(|c| k == c || k.starts_with(&format!("{c}.")))
        })
        .map(|(k, v)| (k.clone(), display_value(v)))
        .collect()
}

fn width(items: impl Iterator<Item = usize>) -> usize {
    items.max().unwrap_or(0)
}

fn render_pretty(caption: &str, shown: &[&Record], selected: &[String]) -> String {
    if shown.is_empty() {
        return format!("{caption}\n\n(no records match the current filter)");
    }
    // A column only appears if some record actually has it.
    let has_time = shown.iter().any(|r| r.time.is_some());
    let has_level = shown.iter().any(|r| r.level_text.is_some());
    let rows: Vec<(String, String, String, Vec<(String, String)>)> = shown
        .iter()
        .map(|r| {
            (
                r.time.clone().unwrap_or_else(|| "-".into()),
                r.level_text.clone().unwrap_or_else(|| "-".into()),
                r.message.clone(),
                extras_for(r, selected),
            )
        })
        .collect();

    let tw = width(rows.iter().map(|(t, ..)| t.chars().count()));
    let lw = width(rows.iter().map(|(_, l, ..)| l.chars().count()));
    let mw = width(rows.iter().map(|(_, _, m, _)| m.chars().count()));

    let mut out = format!("{caption}\n\n");
    for (t, l, m, extras) in &rows {
        let mut line = String::new();
        if has_time {
            let _ = write!(line, "[{t}]{} ", " ".repeat(tw - t.chars().count()));
        }
        if has_level {
            let _ = write!(line, "{l}{} ", " ".repeat(lw - l.chars().count()));
        }
        let _ = write!(line, "{m}{}", " ".repeat(mw - m.chars().count()));
        for (k, v) in extras {
            let _ = write!(line, " {k}={}", quote_pair(v));
        }
        out.push_str(line.trim_end());
        out.push('\n');
    }
    out.pop();
    out
}

/// `key=value` needs quoting once the value has whitespace or a quote in it.
fn quote_pair(v: &str) -> String {
    if v.is_empty() || v.contains(char::is_whitespace) || v.contains('"') {
        format!("\"{}\"", v.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        v.to_string()
    }
}

fn md_escape(s: &str) -> String {
    s.replace('|', "\\|").replace('\n', " ")
}

fn render_table(caption: &str, shown: &[&Record], cols: &[String]) -> String {
    if shown.is_empty() || cols.is_empty() {
        return format!("{caption}\n\n(no records match the current filter)");
    }
    let cells: Vec<Vec<String>> = shown
        .iter()
        .map(|r| {
            cols.iter()
                .map(|c| md_escape(&display_opt(path_get(&r.value, c))))
                .collect()
        })
        .collect();
    // Pad every column so the raw Markdown is readable as plain text too.
    let widths: Vec<usize> = cols
        .iter()
        .enumerate()
        .map(|(i, c)| {
            width(
                std::iter::once(md_escape(c).chars().count())
                    .chain(cells.iter().map(|row| row[i].chars().count())),
            )
            .max(3)
        })
        .collect();
    let pad = |s: &str, w: usize| format!("{s}{}", " ".repeat(w - s.chars().count()));

    let mut out = format!("{caption}\n\n| ");
    out.push_str(
        &cols
            .iter()
            .enumerate()
            .map(|(i, c)| pad(&md_escape(c), widths[i]))
            .collect::<Vec<_>>()
            .join(" | "),
    );
    out.push_str(" |\n| ");
    out.push_str(
        &widths
            .iter()
            .map(|w| "-".repeat(*w))
            .collect::<Vec<_>>()
            .join(" | "),
    );
    out.push_str(" |\n");
    for row in &cells {
        out.push_str("| ");
        out.push_str(
            &row.iter()
                .enumerate()
                .map(|(i, c)| pad(c, widths[i]))
                .collect::<Vec<_>>()
                .join(" | "),
        );
        out.push_str(" |\n");
    }
    out.pop();
    out
}

fn render_json(shown: &[&Record], cols: &[String], whole_record: bool) -> String {
    let arr: Vec<Value> = shown
        .iter()
        .map(|r| {
            let mut m = Map::new();
            if whole_record {
                // The flattened (or compacted) record, key order preserved.
                for (k, v) in &r.flat {
                    m.insert(k.clone(), v.clone());
                }
            } else {
                for c in cols {
                    m.insert(
                        c.clone(),
                        path_get(&r.value, c).cloned().unwrap_or(Value::Null),
                    );
                }
            }
            Value::Object(m)
        })
        .collect();
    serde_json::to_string_pretty(&Value::Array(arr)).unwrap_or_else(|_| "[]".into())
}

fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn render_csv(shown: &[&Record], cols: &[String]) -> String {
    if cols.is_empty() {
        return String::new();
    }
    let mut out = cols
        .iter()
        .map(|c| csv_escape(c))
        .collect::<Vec<_>>()
        .join(",");
    for r in shown {
        out.push('\n');
        out.push_str(
            &cols
                .iter()
                .map(|c| csv_escape(&display_opt(path_get(&r.value, c))))
                .collect::<Vec<_>>()
                .join(","),
        );
    }
    out
}

// ------------------------------------------------------------------ tests

#[cfg(test)]
mod tests {
    use super::*;

    const LOGS: &str = concat!(
        r#"{"time":"2024-01-01T00:00:00Z","level":"info","msg":"server started","port":8080}"#,
        "\n",
        r#"{"time":"2024-01-01T00:00:05Z","level":"warn","msg":"high latency","ms":900}"#,
        "\n",
        r#"{"time":"2024-01-01T00:00:09Z","level":"error","msg":"db timeout","attempt":3}"#,
    );

    /// Defaults: pretty output, no filters.
    fn pretty(input: &str) -> String {
        format_logs(
            input, "", "", "", "", "", "", "", "", true, "", 0, "pretty",
        )
        .unwrap()
    }

    #[test]
    fn pretty_renders_aligned_lines() {
        let out = pretty(LOGS);
        assert_eq!(
            out,
            "3 records\n\n\
             [2024-01-01T00:00:00Z] INFO  server started port=8080\n\
             [2024-01-01T00:00:05Z] WARN  high latency   ms=900\n\
             [2024-01-01T00:00:09Z] ERROR db timeout     attempt=3"
        );
    }

    #[test]
    fn blank_and_comment_lines_are_skipped() {
        let input = format!("# a comment\n\n// another\n{LOGS}\n\n");
        let out = pretty(&input);
        assert!(out.starts_with("3 records\n\n"), "{out}");
    }

    #[test]
    fn crlf_input_parses() {
        let input = LOGS.replace('\n', "\r\n");
        assert!(pretty(&input).starts_with("3 records\n\n"));
    }

    #[test]
    fn nested_objects_flatten_to_dotted_keys() {
        let input = r#"{"level":"info","msg":"req","req":{"method":"GET","url":"/x"},"tags":["a","b"]}"#;
        let out = pretty(input);
        assert!(out.contains("req.method=GET"), "{out}");
        assert!(out.contains("req.url=/x"), "{out}");
        assert!(out.contains("tags.0=a tags.1=b"), "{out}");
    }

    #[test]
    fn flatten_off_compacts_nested_values() {
        let input = r#"{"level":"info","msg":"req","req":{"method":"GET","url":"/x"}}"#;
        let out = format_logs(
            input, "", "", "", "", "", "", "", "", false, "", 0, "pretty",
        )
        .unwrap();
        assert!(out.contains(r#"req="{\"method\":\"GET\",\"url\":\"/x\"}""#), "{out}");
    }

    #[test]
    fn minimum_severity_filter() {
        let out = format_logs(LOGS, "warn", "", "", "", "", "", "", "", true, "", 0, "pretty")
            .unwrap();
        assert!(out.starts_with("3 records · 2 shown"), "{out}");
        assert!(!out.contains("server started"), "{out}");
        assert!(out.contains("db timeout"), "{out}");
    }

    #[test]
    fn bunyan_numeric_levels_map_to_severity() {
        let input = concat!(
            r#"{"level":30,"msg":"info line"}"#,
            "\n",
            r#"{"level":50,"msg":"error line"}"#,
            "\n",
            r#"{"level":60,"msg":"fatal line"}"#,
        );
        let out =
            format_logs(input, "error", "", "", "", "", "", "", "", true, "", 0, "pretty").unwrap();
        assert!(out.contains("ERROR error line"), "{out}");
        assert!(out.contains("FATAL fatal line"), "{out}");
        assert!(!out.contains("info line"), "{out}");
    }

    #[test]
    fn syslog_numeric_levels_map_to_severity() {
        let input = concat!(
            r#"{"severity":7,"msg":"debug line"}"#,
            "\n",
            r#"{"severity":4,"msg":"warn line"}"#,
            "\n",
            r#"{"severity":3,"msg":"error line"}"#,
            "\n",
            r#"{"severity":2,"msg":"crit line"}"#,
        );
        let out =
            format_logs(input, "warn", "", "", "", "", "", "", "", true, "", 0, "pretty").unwrap();
        assert!(out.contains("WARN  warn line"), "{out}");
        assert!(out.contains("ERROR error line"), "{out}");
        assert!(out.contains("FATAL crit line"), "{out}");
        assert!(!out.contains("debug line"), "{out}");
    }

    #[test]
    fn unknown_level_words_render_verbatim_and_sort_as_info() {
        let input = r#"{"level":"notice-ish","msg":"custom"}"#;
        let out = pretty(input);
        assert!(out.contains("NOTICE-ISH custom"), "{out}");
        assert!(
            format_logs(input, "warn", "", "", "", "", "", "", "", true, "", 0, "pretty")
                .unwrap()
                .contains("(no records match")
        );
    }

    #[test]
    fn whole_record_contains_filter_is_case_insensitive() {
        let out = format_logs(LOGS, "", "", "TIMEOUT", "contains", "", "", "", "", true, "", 0, "pretty")
            .unwrap();
        assert!(out.starts_with("3 records · 1 shown"), "{out}");
        assert!(out.contains("db timeout"), "{out}");
    }

    #[test]
    fn dotted_field_filter_contains_and_exact() {
        let input = concat!(
            r#"{"level":"info","msg":"a","req":{"method":"GET"}}"#,
            "\n",
            r#"{"level":"info","msg":"b","req":{"method":"POST"}}"#,
        );
        let contains = format_logs(
            input, "", "req.method", "get", "contains", "", "", "", "", true, "", 0, "pretty",
        )
        .unwrap();
        assert!(contains.contains("INFO a"), "{contains}");
        assert!(!contains.contains("INFO b"), "{contains}");

        // exact is case-sensitive and compares the whole stringified value
        let exact = format_logs(
            input, "", "req.method", "get", "exact", "", "", "", "", true, "", 0, "pretty",
        )
        .unwrap();
        assert!(exact.contains("(no records match"), "{exact}");
        let exact = format_logs(
            input, "", "req.method", "POST", "exact", "", "", "", "", true, "", 0, "pretty",
        )
        .unwrap();
        assert!(exact.contains("INFO b") && !exact.contains("INFO a"), "{exact}");
    }

    #[test]
    fn missing_filter_path_matches_nothing() {
        let out = format_logs(LOGS, "", "nope.here", "x", "contains", "", "", "", "", true, "", 0, "pretty")
            .unwrap();
        assert!(out.contains("(no records match"), "{out}");
    }

    #[test]
    fn custom_key_names() {
        let input = r#"{"ts_custom":1704067200,"sev":"error","body_text":"boom","x":1}"#;
        let out = format_logs(
            input, "", "", "", "", "", "sev", "ts_custom", "body_text", true, "", 0, "pretty",
        )
        .unwrap();
        assert!(out.contains("[2024-01-01T00:00:00Z] ERROR boom x=1"), "{out}");
    }

    #[test]
    fn epoch_millis_render_as_iso_utc() {
        let input = r#"{"time":1704067200123,"level":"info","msg":"ms"}"#;
        assert!(pretty(input).contains("[2024-01-01T00:00:00.123Z]"));
    }

    #[test]
    fn field_selection_orders_columns_and_blanks_missing_paths() {
        let out = format_logs(
            LOGS, "", "", "", "", "msg, port", "", "", "", true, "", 0, "table",
        )
        .unwrap();
        assert_eq!(
            out,
            "3 records\n\n\
             | msg            | port |\n\
             | -------------- | ---- |\n\
             | server started | 8080 |\n\
             | high latency   |      |\n\
             | db timeout     |      |"
        );
    }

    #[test]
    fn table_output_unions_keys_in_first_seen_order() {
        let out = format_logs(LOGS, "", "", "", "", "", "", "", "", true, "", 0, "table").unwrap();
        assert!(out.contains("| time                 | level | msg            | port | ms  | attempt |"));
    }

    #[test]
    fn json_output_is_an_array_of_flattened_records() {
        let input = r#"{"level":"info","msg":"req","req":{"method":"GET"}}"#;
        let out = format_logs(input, "", "", "", "", "", "", "", "", true, "", 0, "json").unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v[0]["req.method"], Value::String("GET".into()));
        assert_eq!(v.as_array().unwrap().len(), 1);
    }

    #[test]
    fn csv_output_has_a_header_and_quotes_commas() {
        let input = r#"{"level":"info","msg":"a, b","n":1}"#;
        let out = format_logs(input, "", "", "", "", "", "", "", "", true, "", 0, "csv").unwrap();
        assert_eq!(out, "level,msg,n\ninfo,\"a, b\",1");
    }

    #[test]
    fn on_invalid_skip_is_the_default_and_is_reported() {
        let input = format!("not json\n{LOGS}");
        let out = pretty(&input);
        assert!(out.starts_with("3 records · 1 invalid line skipped"), "{out}");
    }

    #[test]
    fn on_invalid_keep_passes_the_raw_line_through() {
        let input = format!("not json\n{LOGS}");
        let out =
            format_logs(&input, "", "", "", "", "", "", "", "", true, "keep", 0, "pretty").unwrap();
        assert!(out.starts_with("4 records · 1 invalid line kept"), "{out}");
        assert!(out.contains("not json"), "{out}");
    }

    #[test]
    fn on_invalid_error_names_the_line_number() {
        let input = format!("{LOGS}\nnot json");
        let err = format_logs(&input, "", "", "", "", "", "", "", "", true, "error", 0, "pretty")
            .unwrap_err();
        assert!(err.starts_with("line 4 is not a JSON object"), "{err}");
    }

    #[test]
    fn a_json_array_line_is_not_a_record() {
        let err = format_logs("[1,2,3]", "", "", "", "", "", "", "", "", true, "error", 0, "pretty")
            .unwrap_err();
        assert!(err.starts_with("line 1 is not a JSON object"), "{err}");
    }

    #[test]
    fn limit_caps_rendered_records() {
        let out = format_logs(LOGS, "", "", "", "", "", "", "", "", true, "", 2, "pretty").unwrap();
        assert!(out.starts_with("3 records · 2 shown"), "{out}");
        assert!(!out.contains("db timeout"), "{out}");
    }

    #[test]
    fn limit_at_and_over_the_cap_boundary() {
        let many: String = (0..12)
            .map(|i| format!("{{\"level\":\"info\",\"msg\":\"m{i}\"}}\n"))
            .collect();
        // exactly at the record count → nothing dropped, no "shown" segment
        let at = format_logs(&many, "", "", "", "", "", "", "", "", true, "", 12, "csv").unwrap();
        assert_eq!(at.lines().count(), 13);
        // one under → capped
        let under = format_logs(&many, "", "", "", "", "", "", "", "", true, "", 11, "csv").unwrap();
        assert_eq!(under.lines().count(), 12);
        // above MAX_LIMIT clamps instead of erroring
        let over =
            format_logs(&many, "", "", "", "", "", "", "", "", true, "", MAX_LIMIT + 1, "csv")
                .unwrap();
        assert_eq!(over.lines().count(), 13);
    }

    #[test]
    fn empty_input_errors() {
        assert!(format_logs("  \n ", "", "", "", "", "", "", "", "", true, "", 0, "pretty")
            .unwrap_err()
            .contains("no input"));
    }

    #[test]
    fn only_comments_errors() {
        assert!(
            format_logs("# hi\n// there", "", "", "", "", "", "", "", "", true, "", 0, "pretty")
                .unwrap_err()
                .contains("no JSON records")
        );
    }

    #[test]
    fn bad_enum_values_error() {
        let bad = |l, m, oi, o| {
            format_logs(LOGS, l, "", "", m, "", "", "", "", true, oi, 0, o).unwrap_err()
        };
        assert!(bad("nope", "", "", "pretty").contains("unknown level"));
        assert!(bad("", "nope", "", "pretty").contains("unknown match"));
        assert!(bad("", "", "nope", "pretty").contains("unknown on_invalid"));
        assert!(bad("", "", "", "nope").contains("unknown output"));
    }
}
