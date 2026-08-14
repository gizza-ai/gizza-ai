//! worklog-summarizer core — pure compute, shared by the chat skill block and the web page.
//!
//! Parses a timestamped activity ("doing") log where each entry runs until the next
//! timestamp, then totals the time per project, per tag, per day, or per entry.
//! No wafer/wasm-bindgen deps, no clock, no I/O — fully deterministic.

use std::collections::BTreeMap;
use std::fmt::Write as _;

/// Hard cap on the pasted log, mirrored in the descriptor + page copy.
pub const MAX_INPUT_BYTES: usize = 5_000_000;

const BAR_WIDTH: usize = 24;

/// Words that close the running entry without starting a new one.
const STOP_WORDS: [&str; 10] = [
    "done", "end", "stop", "off", "out", "eod", "finish", "finished", "---", "--",
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GroupBy {
    All,
    Project,
    Tag,
    Day,
    Entry,
}

impl GroupBy {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "all" | "" => Ok(GroupBy::All),
            "project" => Ok(GroupBy::Project),
            "tag" => Ok(GroupBy::Tag),
            "day" => Ok(GroupBy::Day),
            "entry" => Ok(GroupBy::Entry),
            other => Err(format!(
                "unknown group_by '{other}': expected all, project, tag, day or entry"
            )),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OutputFormat {
    Summary,
    Table,
    Csv,
    Json,
}

impl OutputFormat {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "summary" | "" => Ok(OutputFormat::Summary),
            "table" => Ok(OutputFormat::Table),
            "csv" => Ok(OutputFormat::Csv),
            "json" => Ok(OutputFormat::Json),
            other => Err(format!(
                "unknown output '{other}': expected summary, table, csv or json"
            )),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Units {
    Hm,
    Decimal,
    Minutes,
}

impl Units {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "hm" | "" => Ok(Units::Hm),
            "decimal" => Ok(Units::Decimal),
            "minutes" => Ok(Units::Minutes),
            other => Err(format!(
                "unknown units '{other}': expected hm, decimal or minutes"
            )),
        }
    }

    /// Render a duration in minutes using this unit.
    pub fn render(self, minutes: i64) -> String {
        match self {
            Units::Hm => {
                let h = minutes / 60;
                let m = minutes % 60;
                if h > 0 && m > 0 {
                    format!("{h}h {m}m")
                } else if h > 0 {
                    format!("{h}h")
                } else {
                    format!("{m}m")
                }
            }
            Units::Decimal => format!("{:.2}", minutes as f64 / 60.0),
            Units::Minutes => format!("{minutes}"),
        }
    }

    fn header(self) -> &'static str {
        match self {
            Units::Hm => "time",
            Units::Decimal => "hours",
            Units::Minutes => "minutes",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SortBy {
    Duration,
    Name,
    Time,
}

impl SortBy {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "duration" | "" => Ok(SortBy::Duration),
            "name" => Ok(SortBy::Name),
            "time" => Ok(SortBy::Time),
            other => Err(format!(
                "unknown sort '{other}': expected duration, name or time"
            )),
        }
    }
}

/// Every knob the summarizer accepts. Each surface (chat block, CLI, page)
/// builds one of these from its own argument shape.
#[derive(Clone, Debug)]
pub struct Options {
    pub group_by: GroupBy,
    pub output: OutputFormat,
    pub units: Units,
    pub round: i64,
    /// Cap any single entry at this many minutes (0 = no cap). Keeps an
    /// unclosed end-of-day entry from swallowing the whole night.
    pub max_entry: i64,
    pub end_time: String,
    pub from: String,
    pub to: String,
    pub filter: String,
    pub default_project: String,
    pub sort: SortBy,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            group_by: GroupBy::All,
            output: OutputFormat::Summary,
            units: Units::Hm,
            round: 0,
            max_entry: 0,
            end_time: String::new(),
            from: String::new(),
            to: String::new(),
            filter: String::new(),
            default_project: "(untagged)".into(),
            sort: SortBy::Duration,
        }
    }
}

/// One parsed log entry, after durations have been resolved.
#[derive(Clone, Debug)]
struct Entry {
    /// Absolute minute on the parsed timeline (day_index * 1440 + minute of day).
    start_abs: i64,
    /// Clock time of the start, as `HH:MM`.
    start_clock: String,
    /// Day label — the ISO date when the log carries dates, else `(no date)`.
    day: String,
    /// ISO date used for range filtering (empty for undated logs).
    iso_date: String,
    project: String,
    tags: Vec<String>,
    text: String,
    minutes: i64,
    open: bool,
}

/// One aggregated row of the report.
#[derive(Clone, Debug)]
struct Row {
    name: String,
    minutes: i64,
    entries: usize,
    /// First start on the timeline, used by `sort = time`.
    first_abs: i64,
}

/// Summarize a worklog. Returns the rendered report in the requested format.
pub fn summarize(log: &str, opts: &Options) -> Result<String, String> {
    if log.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "worklog too large: {} bytes (maximum {} bytes)",
            log.len(),
            MAX_INPUT_BYTES
        ));
    }
    if log.trim().is_empty() {
        return Err("worklog is empty: paste at least one timestamped line, e.g. '2024-01-15 09:00 @acme writing the parser'".into());
    }
    if opts.round < 0 {
        return Err("round must be 0 or a positive number of minutes".into());
    }
    if opts.max_entry < 0 {
        return Err("max_entry must be 0 (no cap) or a positive number of minutes".into());
    }
    let from = parse_date_bound(&opts.from, "from")?;
    let to = parse_date_bound(&opts.to, "to")?;
    if let (Some(f), Some(t)) = (from.as_ref(), to.as_ref()) {
        if f > t {
            return Err(format!("from date {f} is after to date {t}"));
        }
    }
    let end_time = if opts.end_time.trim().is_empty() {
        None
    } else {
        Some(parse_clock(opts.end_time.trim()).ok_or_else(|| {
            format!(
                "could not read end_time '{}': use a 24-hour HH:MM time like 17:30, or 5:30pm",
                opts.end_time.trim()
            )
        })?)
    };
    let default_project = if opts.default_project.trim().is_empty() {
        "(untagged)".to_string()
    } else {
        opts.default_project.trim().to_string()
    };

    let mut entries = parse_entries(log, &default_project, end_time)?;
    if entries.is_empty() {
        return Err("no timestamped entries found: each line needs a time, e.g. '09:00 @acme writing the parser' or '2024-01-15 09:00 standup'".into());
    }
    let parsed_total = entries.len();
    let open_entries = entries.iter().filter(|e| e.open).count();

    // Cap runaway entries (a day left unclosed otherwise absorbs the night).
    let mut capped = 0usize;
    if opts.max_entry > 0 {
        for e in entries.iter_mut() {
            if e.minutes > opts.max_entry {
                e.minutes = opts.max_entry;
                capped += 1;
            }
        }
    }

    // Round each entry before aggregating so per-entry increments add up the
    // way a billing increment is expected to.
    if opts.round > 0 {
        for e in entries.iter_mut() {
            e.minutes = round_to(e.minutes, opts.round);
        }
    }

    // Date-range filter.
    if from.is_some() || to.is_some() {
        entries.retain(|e| {
            if e.iso_date.is_empty() {
                return false;
            }
            if let Some(f) = from.as_ref() {
                if &e.iso_date < f {
                    return false;
                }
            }
            if let Some(t) = to.as_ref() {
                if &e.iso_date > t {
                    return false;
                }
            }
            true
        });
    }

    // Project/tag filter.
    let patterns = parse_filter(&opts.filter);
    if !patterns.is_empty() {
        entries.retain(|e| {
            matches_any(&e.project, &patterns) || e.tags.iter().any(|t| matches_any(t, &patterns))
        });
    }
    let kept = entries.len();
    if kept == 0 {
        return Err(
            "no entries left after the from/to and filter options — widen the range or clear the filter"
                .into(),
        );
    }

    let total: i64 = entries.iter().map(|e| e.minutes).sum();
    let projects = aggregate(&entries, |e| vec![e.project.clone()], opts.sort);
    let tags = aggregate(&entries, |e| e.tags.clone(), opts.sort);
    let days = aggregate(&entries, |e| vec![e.day.clone()], opts.sort);

    let stats = Stats {
        parsed_total,
        kept,
        open_entries,
        capped,
        total,
    };
    Ok(match opts.output {
        OutputFormat::Summary => {
            render_summary(&entries, &projects, &tags, &days, &stats, opts)
        }
        OutputFormat::Table => render_table(&entries, &projects, &tags, &days, &stats, opts),
        OutputFormat::Csv => render_csv(&entries, &projects, &tags, &days, &stats, opts),
        OutputFormat::Json => render_json(&entries, &projects, &tags, &days, &stats, opts),
    })
}

/// Default one-argument entry point used by simple callers.
pub fn run(log: &str) -> Result<String, String> {
    summarize(log, &Options::default())
}

struct Stats {
    parsed_total: usize,
    kept: usize,
    open_entries: usize,
    capped: usize,
    total: i64,
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/// A timestamp read off one line, before durations are known.
struct RawLine {
    iso_date: Option<String>,
    minute_of_day: i64,
    clock: String,
    text: String,
}

fn parse_entries(
    log: &str,
    default_project: &str,
    end_time: Option<i64>,
) -> Result<Vec<Entry>, String> {
    let mut raw: Vec<RawLine> = Vec::new();
    for line in log.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("//") {
            continue;
        }
        if let Some(r) = parse_line(trimmed) {
            raw.push(r);
        }
    }
    if raw.is_empty() {
        return Ok(Vec::new());
    }

    // Resolve each line onto an absolute-minute timeline: dated lines set the
    // current day; dateless lines inherit it and roll past midnight whenever
    // the clock goes backwards.
    let mut day_index: i64 = 0;
    let mut current_date: Option<String> = None;
    let mut last_minute: Option<i64> = None;
    let mut placed: Vec<(i64, RawLine, String, String)> = Vec::new(); // (abs, line, day label, iso)
    for r in raw {
        match (&r.iso_date, &current_date) {
            // An explicit date always wins: it sets (or advances) the day.
            (Some(d), Some(cur)) => {
                if d != cur {
                    day_index += days_between(cur, d).unwrap_or(1).max(1);
                    current_date = Some(d.clone());
                }
            }
            (Some(d), None) => {
                current_date = Some(d.clone());
            }
            // A dateless line inherits the current day and rolls past midnight
            // whenever the clock goes backwards.
            (None, _) => {
                if let Some(prev) = last_minute {
                    if r.minute_of_day < prev {
                        day_index += 1;
                        if let Some(cur) = current_date.clone() {
                            current_date = Some(add_days(&cur, 1));
                        }
                            }
                }
            }
        }
        last_minute = Some(r.minute_of_day);
        let abs = day_index * 1440 + r.minute_of_day;
        let iso = current_date.clone().unwrap_or_default();
        let day_label = if iso.is_empty() {
            "(no date)".to_string()
        } else {
            iso.clone()
        };
        placed.push((abs, r, day_label, iso));
    }

    // Each entry runs until the next timestamp; stop markers only close.
    let mut entries: Vec<Entry> = Vec::new();
    for i in 0..placed.len() {
        let (abs, ref r, ref day, ref iso) = placed[i];
        let body = r.text.trim();
        if is_stop_marker(body) {
            continue;
        }
        let next_abs = placed.get(i + 1).map(|p| p.0);
        let (minutes, open) = match next_abs {
            Some(n) if n >= abs => ((n - abs), false),
            Some(_) => (0, false),
            None => match end_time {
                Some(t) => {
                    let mut end_abs = (abs / 1440) * 1440 + t;
                    if end_abs < abs {
                        end_abs += 1440;
                    }
                    (end_abs - abs, false)
                }
                None => (0, true),
            },
        };
        let (project, tags, text) = split_tags(body, default_project);
        entries.push(Entry {
            start_abs: abs,
            start_clock: r.clock.clone(),
            day: day.clone(),
            iso_date: iso.clone(),
            project,
            tags,
            text,
            minutes,
            open,
        });
    }
    Ok(entries)
}

/// Pull `[date] time rest` off one line. Returns `None` for lines with no time.
fn parse_line(line: &str) -> Option<RawLine> {
    // Strip a leading bracketed timestamp block: `[2024-01-15 09:00] rest`.
    let (stamp_area, rest_after_bracket) = if let Some(stripped) = line.strip_prefix('[') {
        match stripped.find(']') {
            Some(idx) => (&stripped[..idx], Some(stripped[idx + 1..].to_string())),
            None => (line, None),
        }
    } else {
        (line, None)
    };

    let mut cursor = stamp_area.trim();
    let mut iso_date: Option<String> = None;

    // Optional leading date (YYYY-MM-DD, YYYY/MM/DD, or ISO `YYYY-MM-DDTHH:MM`).
    let first_token_end = cursor.find(char::is_whitespace).unwrap_or(cursor.len());
    let first = &cursor[..first_token_end];
    if let Some((d, time_part)) = split_iso_datetime(first) {
        iso_date = Some(d);
        if let Some(t) = time_part {
            let minute = parse_clock(&t)?;
            let text = match rest_after_bracket {
                Some(r) => r,
                None => cursor[first_token_end..].to_string(),
            };
            return Some(RawLine {
                iso_date,
                minute_of_day: minute,
                clock: fmt_clock(minute),
                text: strip_separator(&text),
            });
        }
        cursor = cursor[first_token_end..].trim_start();
    }

    // The time token (plus an optional `am`/`pm` written as a separate word).
    let time_end = cursor.find(char::is_whitespace).unwrap_or(cursor.len());
    let mut time_token = cursor[..time_end].to_string();
    let mut after = cursor[time_end..].trim_start().to_string();
    let lower_after = after.to_ascii_lowercase();
    for suffix in ["am", "pm", "a.m.", "p.m."] {
        if lower_after == suffix || lower_after.starts_with(&format!("{suffix} ")) {
            time_token.push_str(suffix);
            after = after[suffix.len()..].trim_start().to_string();
            break;
        }
    }
    let minute = parse_clock(&time_token)?;
    let text = match rest_after_bracket {
        Some(r) => r,
        None => after,
    };
    Some(RawLine {
        iso_date,
        minute_of_day: minute,
        clock: fmt_clock(minute),
        text: strip_separator(&text),
    })
}

/// `2024-01-15` → (date, None); `2024-01-15T09:30` → (date, Some("09:30")).
fn split_iso_datetime(token: &str) -> Option<(String, Option<String>)> {
    let (date_part, time_part) = match token.find(['T', 't']) {
        Some(idx) if idx == 10 => (&token[..idx], Some(token[idx + 1..].to_string())),
        _ => (token, None),
    };
    let norm = date_part.replace('/', "-");
    let bits: Vec<&str> = norm.split('-').collect();
    if bits.len() != 3 {
        return None;
    }
    let y: i64 = bits[0].parse().ok()?;
    let m: i64 = bits[1].parse().ok()?;
    let d: i64 = bits[2].parse().ok()?;
    if bits[0].len() != 4 || !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    Some((format!("{y:04}-{m:02}-{d:02}"), time_part))
}

/// `09:00`, `9:00`, `0900`, `9am`, `5:30pm`, `09:00:15` → minutes since midnight.
fn parse_clock(token: &str) -> Option<i64> {
    let t = token.trim().trim_end_matches([',', ';']).to_ascii_lowercase();
    let t = t.replace("a.m.", "am").replace("p.m.", "pm");
    let (body, meridiem) = if let Some(b) = t.strip_suffix("am") {
        (b.trim().to_string(), Some(false))
    } else if let Some(b) = t.strip_suffix("pm") {
        (b.trim().to_string(), Some(true))
    } else {
        (t, None)
    };
    if body.is_empty() {
        return None;
    }
    let (h, m) = if let Some((hs, rest)) = body.split_once(':') {
        let ms = rest.split(':').next().unwrap_or("0");
        (hs.parse::<i64>().ok()?, ms.parse::<i64>().ok()?)
    } else if body.len() == 4 && body.chars().all(|c| c.is_ascii_digit()) {
        (body[..2].parse().ok()?, body[2..].parse().ok()?)
    } else if meridiem.is_some() && body.chars().all(|c| c.is_ascii_digit()) {
        (body.parse::<i64>().ok()?, 0)
    } else {
        return None;
    };
    if !(0..=59).contains(&m) {
        return None;
    }
    let h = match meridiem {
        Some(true) if h < 12 => h + 12,
        Some(false) if h == 12 => 0,
        _ => h,
    };
    if !(0..=24).contains(&h) {
        return None;
    }
    Some(h * 60 + m)
}

fn fmt_clock(minute_of_day: i64) -> String {
    format!("{:02}:{:02}", minute_of_day / 60, minute_of_day % 60)
}

/// Drop a leading `-`, `–`, `|` or `:` separator between the time and the text.
fn strip_separator(text: &str) -> String {
    let t = text.trim_start();
    for sep in ['-', '–', '—', '|', ':', '\t'] {
        if let Some(rest) = t.strip_prefix(sep) {
            return rest.trim().to_string();
        }
    }
    t.trim().to_string()
}

fn is_stop_marker(body: &str) -> bool {
    let cleaned: String = body
        .chars()
        .filter(|c| !matches!(c, '.' | '!' | '*'))
        .collect();
    let cleaned = cleaned.trim().to_ascii_lowercase();
    cleaned.is_empty() || STOP_WORDS.contains(&cleaned.as_str())
}

/// Split an entry body into (project, tags, remaining text). `@name` wins as
/// the project, else the first `+name`/`#name`; every tagged token is a tag.
fn split_tags(body: &str, default_project: &str) -> (String, Vec<String>, String) {
    let mut tags: Vec<String> = Vec::new();
    let mut at_project: Option<String> = None;
    let mut plus_project: Option<String> = None;
    let mut words: Vec<&str> = Vec::new();
    for word in body.split_whitespace() {
        let cleaned = word.trim_end_matches([',', '.', ';', ':']);
        let is_tag = cleaned.len() > 1 && matches!(cleaned.chars().next(), Some('@' | '+' | '#'));
        if is_tag {
            let tag = cleaned.to_string();
            if tag.starts_with('@') && at_project.is_none() {
                at_project = Some(tag.clone());
            }
            if !tag.starts_with('@') && plus_project.is_none() {
                plus_project = Some(tag.clone());
            }
            if !tags.contains(&tag) {
                tags.push(tag);
            }
        } else {
            words.push(word);
        }
    }
    let project = at_project
        .or(plus_project)
        .unwrap_or_else(|| default_project.to_string());
    let text = if words.is_empty() {
        body.trim().to_string()
    } else {
        words.join(" ")
    };
    (project, tags, text)
}

fn parse_date_bound(raw: &str, label: &str) -> Result<Option<String>, String> {
    let t = raw.trim();
    if t.is_empty() {
        return Ok(None);
    }
    match split_iso_datetime(t) {
        Some((d, _)) => Ok(Some(d)),
        None => Err(format!(
            "could not read {label} date '{t}': use YYYY-MM-DD, e.g. 2024-01-15"
        )),
    }
}

fn parse_filter(raw: &str) -> Vec<String> {
    raw.split([',', '\n'])
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .collect()
}

fn matches_any(value: &str, patterns: &[String]) -> bool {
    let v = value.to_ascii_lowercase();
    let bare = v.trim_start_matches(['@', '+', '#']);
    patterns.iter().any(|p| {
        let pb = p.trim_start_matches(['@', '+', '#']);
        if let Some(prefix) = pb.strip_suffix('*') {
            bare.starts_with(prefix)
        } else {
            bare == pb
        }
    })
}

fn round_to(minutes: i64, increment: i64) -> i64 {
    if increment <= 1 {
        return minutes;
    }
    let half = increment / 2;
    ((minutes + half) / increment) * increment
}

// ---------------------------------------------------------------------------
// Calendar helpers (no chrono — days since the civil epoch, Howard Hinnant's algorithm)
// ---------------------------------------------------------------------------

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn parse_ymd(date: &str) -> Option<(i64, i64, i64)> {
    let bits: Vec<&str> = date.split('-').collect();
    if bits.len() != 3 {
        return None;
    }
    Some((
        bits[0].parse().ok()?,
        bits[1].parse().ok()?,
        bits[2].parse().ok()?,
    ))
}

fn days_between(a: &str, b: &str) -> Option<i64> {
    let (ay, am, ad) = parse_ymd(a)?;
    let (by, bm, bd) = parse_ymd(b)?;
    Some(days_from_civil(by, bm, bd) - days_from_civil(ay, am, ad))
}

fn add_days(date: &str, n: i64) -> String {
    match parse_ymd(date) {
        Some((y, m, d)) => {
            let (ny, nm, nd) = civil_from_days(days_from_civil(y, m, d) + n);
            format!("{ny:04}-{nm:02}-{nd:02}")
        }
        None => date.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Aggregation + rendering
// ---------------------------------------------------------------------------

fn aggregate<F>(entries: &[Entry], key: F, sort: SortBy) -> Vec<Row>
where
    F: Fn(&Entry) -> Vec<String>,
{
    let mut acc: BTreeMap<String, Row> = BTreeMap::new();
    for e in entries {
        for k in key(e) {
            let row = acc.entry(k.clone()).or_insert(Row {
                name: k,
                minutes: 0,
                entries: 0,
                first_abs: e.start_abs,
            });
            row.minutes += e.minutes;
            row.entries += 1;
            row.first_abs = row.first_abs.min(e.start_abs);
        }
    }
    let mut rows: Vec<Row> = acc.into_values().collect();
    match sort {
        SortBy::Duration => rows.sort_by(|a, b| {
            b.minutes
                .cmp(&a.minutes)
                .then_with(|| a.name.cmp(&b.name))
        }),
        SortBy::Name => rows.sort_by(|a, b| a.name.cmp(&b.name)),
        SortBy::Time => rows.sort_by(|a, b| {
            a.first_abs
                .cmp(&b.first_abs)
                .then_with(|| a.name.cmp(&b.name))
        }),
    }
    rows
}

fn percent(minutes: i64, total: i64) -> f64 {
    if total == 0 {
        0.0
    } else {
        minutes as f64 * 100.0 / total as f64
    }
}

fn bar(minutes: i64, max: i64) -> String {
    if max <= 0 {
        return String::new();
    }
    let filled = ((minutes as f64 / max as f64) * BAR_WIDTH as f64).round() as usize;
    let filled = filled.min(BAR_WIDTH);
    format!("{}{}", "#".repeat(filled), "-".repeat(BAR_WIDTH - filled))
}

fn section(out: &mut String, title: &str, rows: &[Row], total: i64, units: Units) {
    let _ = writeln!(out, "{title}");
    if rows.is_empty() {
        let _ = writeln!(out, "  (none)");
        let _ = writeln!(out);
        return;
    }
    let max = rows.iter().map(|r| r.minutes).max().unwrap_or(0);
    let name_w = rows.iter().map(|r| r.name.chars().count()).max().unwrap_or(1);
    let val_w = rows
        .iter()
        .map(|r| units.render(r.minutes).chars().count())
        .max()
        .unwrap_or(1);
    for r in rows {
        let _ = writeln!(
            out,
            "  {:<name_w$}  {:>val_w$}  {:>5.1}%  {}",
            r.name,
            units.render(r.minutes),
            percent(r.minutes, total),
            bar(r.minutes, max),
            name_w = name_w,
            val_w = val_w,
        );
    }
    let _ = writeln!(out);
}

fn render_summary(
    entries: &[Entry],
    projects: &[Row],
    tags: &[Row],
    days: &[Row],
    stats: &Stats,
    opts: &Options,
) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "Worklog summary");
    let _ = writeln!(out, "===============");
    let _ = writeln!(out, "Entries: {}", stats.kept);
    let _ = writeln!(out, "Days: {}", days.len());
    let _ = writeln!(
        out,
        "Tracked: {} ({})",
        opts.units.render(stats.total),
        opts.units.header()
    );
    if let (Some(first), Some(last)) = (entries.first(), entries.last()) {
        let _ = writeln!(
            out,
            "Span: {} {} → {} {}",
            first.day, first.start_clock, last.day, last.start_clock
        );
    }
    if opts.round > 0 {
        let _ = writeln!(out, "Rounding: {} minute increments", opts.round);
    }
    if stats.kept < stats.parsed_total {
        let _ = writeln!(
            out,
            "Filtered out: {} of {} entries",
            stats.parsed_total - stats.kept,
            stats.parsed_total
        );
    }
    if stats.open_entries > 0 {
        let _ = writeln!(
            out,
            "Open entries: {} (still running — set an end time to close the last entry)",
            stats.open_entries
        );
    }
    if stats.capped > 0 {
        let _ = writeln!(out, "Capped entries: {}", stats.capped);
    }
    let _ = writeln!(out);

    match opts.group_by {
        GroupBy::All => {
            section(&mut out, "Time per project", projects, stats.total, opts.units);
            section(&mut out, "Time per tag", tags, stats.total, opts.units);
            section(&mut out, "Time per day", days, stats.total, opts.units);
        }
        GroupBy::Project => section(
            &mut out,
            "Time per project",
            projects,
            stats.total,
            opts.units,
        ),
        GroupBy::Tag => section(&mut out, "Time per tag", tags, stats.total, opts.units),
        GroupBy::Day => section(&mut out, "Time per day", days, stats.total, opts.units),
        GroupBy::Entry => {
            let _ = writeln!(out, "Entries");
            let val_w = entries
                .iter()
                .map(|e| opts.units.render(e.minutes).chars().count())
                .max()
                .unwrap_or(1);
            for e in entries {
                let _ = writeln!(
                    out,
                    "  {} {}  {:>val_w$}  {}  {}{}",
                    e.day,
                    e.start_clock,
                    opts.units.render(e.minutes),
                    e.project,
                    e.text,
                    if e.open { "  [open]" } else { "" },
                    val_w = val_w,
                );
            }
            let _ = writeln!(out);
        }
    }
    out.trim_end().to_string()
}

fn render_table(
    entries: &[Entry],
    projects: &[Row],
    tags: &[Row],
    days: &[Row],
    stats: &Stats,
    opts: &Options,
) -> String {
    let mut out = String::new();
    if opts.group_by == GroupBy::Entry {
        let _ = writeln!(
            out,
            "day\tstart\t{}\tproject\ttags\tentry",
            opts.units.header()
        );
        for e in entries {
            let _ = writeln!(
                out,
                "{}\t{}\t{}\t{}\t{}\t{}",
                e.day,
                e.start_clock,
                opts.units.render(e.minutes),
                e.project,
                e.tags.join(" "),
                e.text
            );
        }
        return out.trim_end().to_string();
    }
    let _ = writeln!(
        out,
        "group\tname\t{}\tpercent\tentries",
        opts.units.header()
    );
    let push = |label: &str, rows: &[Row], out: &mut String| {
        for r in rows {
            let _ = writeln!(
                out,
                "{}\t{}\t{}\t{:.1}\t{}",
                label,
                r.name,
                opts.units.render(r.minutes),
                percent(r.minutes, stats.total),
                r.entries
            );
        }
    };
    match opts.group_by {
        GroupBy::All => {
            push("project", projects, &mut out);
            push("tag", tags, &mut out);
            push("day", days, &mut out);
        }
        GroupBy::Project => push("project", projects, &mut out),
        GroupBy::Tag => push("tag", tags, &mut out),
        GroupBy::Day => push("day", days, &mut out),
        GroupBy::Entry => unreachable!(),
    }
    let _ = writeln!(
        out,
        "total\tall\t{}\t100.0\t{}",
        opts.units.render(stats.total),
        stats.kept
    );
    out.trim_end().to_string()
}

fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\n']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn render_csv(
    entries: &[Entry],
    projects: &[Row],
    tags: &[Row],
    days: &[Row],
    stats: &Stats,
    opts: &Options,
) -> String {
    let mut out = String::new();
    if opts.group_by == GroupBy::Entry {
        let _ = writeln!(
            out,
            "day,start,{},project,tags,entry",
            opts.units.header()
        );
        for e in entries {
            let _ = writeln!(
                out,
                "{},{},{},{},{},{}",
                csv_field(&e.day),
                csv_field(&e.start_clock),
                csv_field(&opts.units.render(e.minutes)),
                csv_field(&e.project),
                csv_field(&e.tags.join(" ")),
                csv_field(&e.text)
            );
        }
        return out.trim_end().to_string();
    }
    let _ = writeln!(out, "group,name,{},percent,entries", opts.units.header());
    let push = |label: &str, rows: &[Row], out: &mut String| {
        for r in rows {
            let _ = writeln!(
                out,
                "{},{},{},{:.1},{}",
                label,
                csv_field(&r.name),
                csv_field(&opts.units.render(r.minutes)),
                percent(r.minutes, stats.total),
                r.entries
            );
        }
    };
    match opts.group_by {
        GroupBy::All => {
            push("project", projects, &mut out);
            push("tag", tags, &mut out);
            push("day", days, &mut out);
        }
        GroupBy::Project => push("project", projects, &mut out),
        GroupBy::Tag => push("tag", tags, &mut out),
        GroupBy::Day => push("day", days, &mut out),
        GroupBy::Entry => unreachable!(),
    }
    let _ = writeln!(
        out,
        "total,all,{},100.0,{}",
        csv_field(&opts.units.render(stats.total)),
        stats.kept
    );
    out.trim_end().to_string()
}

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out
}

fn json_rows(out: &mut String, label: &str, rows: &[Row], total: i64, units: Units, last: bool) {
    let _ = writeln!(out, "  \"{label}\": [");
    for (i, r) in rows.iter().enumerate() {
        let _ = writeln!(
            out,
            "    {{ \"name\": \"{}\", \"minutes\": {}, \"hours\": {:.2}, \"display\": \"{}\", \"percent\": {:.1}, \"entries\": {} }}{}",
            json_escape(&r.name),
            r.minutes,
            r.minutes as f64 / 60.0,
            json_escape(&units.render(r.minutes)),
            percent(r.minutes, total),
            r.entries,
            if i + 1 == rows.len() { "" } else { "," }
        );
    }
    let _ = writeln!(out, "  ]{}", if last { "" } else { "," });
}

fn render_json(
    entries: &[Entry],
    projects: &[Row],
    tags: &[Row],
    days: &[Row],
    stats: &Stats,
    opts: &Options,
) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "{{");
    let _ = writeln!(out, "  \"entries\": {},", stats.kept);
    let _ = writeln!(out, "  \"parsed_entries\": {},", stats.parsed_total);
    let _ = writeln!(out, "  \"open_entries\": {},", stats.open_entries);
    let _ = writeln!(out, "  \"total_minutes\": {},", stats.total);
    let _ = writeln!(
        out,
        "  \"total_hours\": {:.2},",
        stats.total as f64 / 60.0
    );
    let _ = writeln!(
        out,
        "  \"total_display\": \"{}\",",
        json_escape(&opts.units.render(stats.total))
    );
    let _ = writeln!(out, "  \"days_covered\": {},", days.len());
    match opts.group_by {
        GroupBy::All => {
            json_rows(&mut out, "projects", projects, stats.total, opts.units, false);
            json_rows(&mut out, "tags", tags, stats.total, opts.units, false);
            json_rows(&mut out, "days", days, stats.total, opts.units, true);
        }
        GroupBy::Project => {
            json_rows(&mut out, "projects", projects, stats.total, opts.units, true)
        }
        GroupBy::Tag => json_rows(&mut out, "tags", tags, stats.total, opts.units, true),
        GroupBy::Day => json_rows(&mut out, "days", days, stats.total, opts.units, true),
        GroupBy::Entry => {
            let _ = writeln!(out, "  \"entry_list\": [");
            for (i, e) in entries.iter().enumerate() {
                let _ = writeln!(
                    out,
                    "    {{ \"day\": \"{}\", \"start\": \"{}\", \"minutes\": {}, \"display\": \"{}\", \"project\": \"{}\", \"tags\": [{}], \"text\": \"{}\", \"open\": {} }}{}",
                    json_escape(&e.day),
                    json_escape(&e.start_clock),
                    e.minutes,
                    json_escape(&opts.units.render(e.minutes)),
                    json_escape(&e.project),
                    e.tags
                        .iter()
                        .map(|t| format!("\"{}\"", json_escape(t)))
                        .collect::<Vec<_>>()
                        .join(", "),
                    json_escape(&e.text),
                    e.open,
                    if i + 1 == entries.len() { "" } else { "," }
                );
            }
            let _ = writeln!(out, "  ]");
        }
    }
    let _ = write!(out, "}}");
    out
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const LOG: &str = "2024-01-15 09:00 @acme +dev writing the parser\n\
                       2024-01-15 10:30 @acme +review code review\n\
                       2024-01-15 12:00 lunch\n\
                       2024-01-15 13:00 @beta +dev bugfix\n\
                       2024-01-15 17:00 done";

    fn opts() -> Options {
        Options::default()
    }

    #[test]
    fn summary_totals_projects_tags_and_days() {
        let out = summarize(LOG, &opts()).unwrap();
        assert!(out.contains("Entries: 4"), "{out}");
        assert!(out.contains("Tracked: 8h"), "{out}");
        assert!(out.contains("@beta"), "{out}");
        assert!(out.contains("@acme"), "{out}");
        assert!(out.contains("(untagged)"), "{out}");
        assert!(out.contains("+dev"), "{out}");
        assert!(out.contains("2024-01-15"), "{out}");
        // @beta 4h = 50%, @acme 3h = 37.5%, untagged 1h = 12.5%
        assert!(out.contains(" 50.0%"), "{out}");
        assert!(out.contains(" 37.5%"), "{out}");
        assert!(out.contains(" 12.5%"), "{out}");
    }

    #[test]
    fn stop_marker_closes_the_day_without_adding_time() {
        let out = summarize(LOG, &opts()).unwrap();
        assert!(!out.contains("Open entries"), "{out}");
        assert!(!out.contains("done"), "{out}");
    }

    #[test]
    fn open_last_entry_is_flagged_and_end_time_closes_it() {
        let log = "09:00 @acme start\n10:00 @acme still going";
        let out = summarize(log, &opts()).unwrap();
        assert!(out.contains("Open entries: 1"), "{out}");
        assert!(out.contains("Tracked: 1h"), "{out}");

        let mut o = opts();
        o.end_time = "12:30".into();
        let closed = summarize(log, &o).unwrap();
        assert!(!closed.contains("Open entries"), "{closed}");
        assert!(closed.contains("Tracked: 3h 30m"), "{closed}");
    }

    #[test]
    fn json_output_is_machine_readable() {
        let mut o = opts();
        o.output = OutputFormat::Json;
        o.group_by = GroupBy::Project;
        let out = summarize(LOG, &o).unwrap();
        assert!(out.contains("\"total_minutes\": 480"), "{out}");
        assert!(out.contains("\"name\": \"@beta\", \"minutes\": 240"), "{out}");
        assert!(out.contains("\"percent\": 50.0"), "{out}");
    }

    #[test]
    fn csv_and_table_carry_every_grouping() {
        let mut o = opts();
        o.output = OutputFormat::Csv;
        let csv = summarize(LOG, &o).unwrap();
        assert!(csv.starts_with("group,name,time,percent,entries"), "{csv}");
        assert!(csv.contains("project,@acme,3h,37.5,2"), "{csv}");
        assert!(csv.contains("day,2024-01-15,8h,100.0,4"), "{csv}");
        assert!(csv.contains("total,all,8h,100.0,4"), "{csv}");

        o.output = OutputFormat::Table;
        o.group_by = GroupBy::Tag;
        let table = summarize(LOG, &o).unwrap();
        assert!(table.contains("tag\t+dev\t5h 30m\t68.8\t2"), "{table}");
    }

    #[test]
    fn units_render_decimal_and_minutes() {
        let mut o = opts();
        o.units = Units::Decimal;
        o.group_by = GroupBy::Project;
        let dec = summarize(LOG, &o).unwrap();
        assert!(dec.contains("4.00"), "{dec}");
        o.units = Units::Minutes;
        let mins = summarize(LOG, &o).unwrap();
        assert!(mins.contains("Tracked: 480"), "{mins}");
    }

    #[test]
    fn rounding_snaps_each_entry_to_the_increment() {
        let log = "09:00 @acme a\n09:07 @acme b\n09:20 end";
        let mut o = opts();
        o.round = 15;
        o.group_by = GroupBy::Project;
        let out = summarize(log, &o).unwrap();
        // 7m → 0m, 13m → 15m
        assert!(out.contains("Tracked: 15m"), "{out}");
        assert!(out.contains("Rounding: 15 minute increments"), "{out}");
    }

    #[test]
    fn dateless_lines_roll_past_midnight() {
        let log = "22:00 @night shift\n01:00 @night more\n02:00 done";
        let mut o = opts();
        o.group_by = GroupBy::Project;
        let out = summarize(log, &o).unwrap();
        assert!(out.contains("Tracked: 4h"), "{out}");
        assert!(out.contains("(no date)"), "{out}");
    }

    #[test]
    fn date_range_and_filter_narrow_the_report() {
        let log = "2024-01-15 09:00 @acme a\n2024-01-15 11:00 @beta b\n2024-01-16 09:00 @acme c\n2024-01-16 10:00 done";
        let mut o = opts();
        o.from = "2024-01-16".into();
        o.group_by = GroupBy::Project;
        let out = summarize(log, &o).unwrap();
        assert!(out.contains("Tracked: 1h"), "{out}");
        assert!(!out.contains("@beta"), "{out}");

        let mut o2 = opts();
        o2.filter = "beta".into();
        o2.group_by = GroupBy::Project;
        let filtered = summarize(log, &o2).unwrap();
        assert!(filtered.contains("@beta"), "{filtered}");
        assert!(!filtered.contains("@acme"), "{filtered}");

        let mut o3 = opts();
        o3.filter = "ac*".into();
        o3.group_by = GroupBy::Project;
        let prefix = summarize(log, &o3).unwrap();
        assert!(prefix.contains("@acme"), "{prefix}");
        assert!(!prefix.contains("@beta"), "{prefix}");
    }

    #[test]
    fn entry_grouping_lists_each_line() {
        let mut o = opts();
        o.group_by = GroupBy::Entry;
        o.sort = SortBy::Time;
        let out = summarize(LOG, &o).unwrap();
        assert!(out.contains("2024-01-15 09:00"), "{out}");
        assert!(out.contains("writing the parser"), "{out}");
        assert!(out.contains("code review"), "{out}");
    }

    #[test]
    fn assorted_timestamp_dialects_parse() {
        let log = "[2024-01-15 09:00] @acme bracketed\n2024-01-15T10:00 @acme iso\n2024-01-15 11:00am - @acme meridiem\n2024-01-15 1:30pm @acme afternoon\n2024-01-15 14:00 done";
        let mut o = opts();
        o.group_by = GroupBy::Project;
        let out = summarize(log, &o).unwrap();
        assert!(out.contains("Entries: 4"), "{out}");
        assert!(out.contains("Tracked: 5h"), "{out}");
    }

    #[test]
    fn default_project_label_is_configurable() {
        let mut o = opts();
        o.default_project = "personal".into();
        o.group_by = GroupBy::Project;
        let out = summarize(LOG, &o).unwrap();
        assert!(out.contains("personal"), "{out}");
        assert!(!out.contains("(untagged)"), "{out}");
    }

    #[test]
    fn sort_by_name_and_time_reorder_rows() {
        let mut o = opts();
        o.group_by = GroupBy::Project;
        o.sort = SortBy::Name;
        let by_name = summarize(LOG, &o).unwrap();
        let acme = by_name.find("@acme").unwrap();
        let beta = by_name.find("@beta").unwrap();
        assert!(acme < beta, "{by_name}");

        o.sort = SortBy::Duration;
        let by_dur = summarize(LOG, &o).unwrap();
        assert!(
            by_dur.find("@beta").unwrap() < by_dur.find("@acme").unwrap(),
            "{by_dur}"
        );
    }

    #[test]
    fn empty_and_untimed_input_error_clearly() {
        let err = summarize("   ", &opts()).unwrap_err();
        assert!(err.contains("worklog is empty"), "{err}");

        let err = summarize("just some notes\nno times here", &opts()).unwrap_err();
        assert!(err.contains("no timestamped entries"), "{err}");

        let mut o = opts();
        o.from = "nonsense".into();
        let err = summarize(LOG, &o).unwrap_err();
        assert!(err.contains("could not read from date"), "{err}");

        let mut o = opts();
        o.end_time = "hammer time".into();
        let err = summarize(LOG, &o).unwrap_err();
        assert!(err.contains("could not read end_time"), "{err}");

        let mut o = opts();
        o.filter = "nothing-matches".into();
        let err = summarize(LOG, &o).unwrap_err();
        assert!(err.contains("no entries left"), "{err}");
    }

    #[test]
    fn oversized_input_is_rejected() {
        let big = "x".repeat(MAX_INPUT_BYTES + 1);
        let err = summarize(&big, &opts()).unwrap_err();
        assert!(err.contains("worklog too large"), "{err}");
    }

    #[test]
    fn enum_parsers_reject_unknown_values() {
        assert!(GroupBy::parse("nope").is_err());
        assert!(OutputFormat::parse("xml").is_err());
        assert!(Units::parse("fortnights").is_err());
        assert!(SortBy::parse("random").is_err());
        assert_eq!(GroupBy::parse("day").unwrap(), GroupBy::Day);
        assert_eq!(OutputFormat::parse("CSV").unwrap(), OutputFormat::Csv);
    }

    #[test]
    fn comments_and_blank_lines_are_ignored() {
        let log = "# my log\n\n09:00 @acme a\n// note\n10:00 done";
        let mut o = opts();
        o.group_by = GroupBy::Project;
        let out = summarize(log, &o).unwrap();
        assert!(out.contains("Entries: 1"), "{out}");
        assert!(out.contains("Tracked: 1h"), "{out}");
    }
}

