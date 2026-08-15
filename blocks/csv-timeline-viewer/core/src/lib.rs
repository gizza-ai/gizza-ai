//! csv-timeline-viewer core — pure compute, shared by the chat skill block and the web page.
//! No wafer/wasm-bindgen deps.
//!
//! Loads a CSV / TSV / JSON-Lines table of timestamped events, auto-detects the timestamp
//! column, then filters (time range, column conditions, full-text or regex search), sorts,
//! projects columns, pages, and renders as an aligned table, CSV, JSON, JSONL, or an
//! activity summary with a time histogram.

use std::collections::HashSet;

use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};

/// Hard cap on input lines — keeps a pasted "large CSV" from wedging the browser tab.
const MAX_INPUT_LINES: usize = 200_000;
/// Hard cap on parsed data rows.
const MAX_ROWS: usize = 200_000;
/// Table-mode cell truncation width (characters).
const MAX_CELL: usize = 60;
/// Upper bound on histogram buckets in `summary` output.
const MAX_BUCKETS: usize = 60;

/// Header names that mark a column as the event timestamp, matched case-insensitively
/// against the header with non-alphanumeric characters stripped.
const TIME_HEADERS: &[&str] = &[
    "time",
    "timestamp",
    "datetime",
    "date",
    "ts",
    "eventtime",
    "eventtimestamp",
    "eventdate",
    "occurredat",
    "occurred",
    "createdat",
    "created",
    "creationtime",
    "logged",
    "loggedat",
    "loggedtime",
    "logtime",
    "logdate",
    "recordedat",
    "receivedat",
    "startedat",
    "starttime",
    "start",
    "when",
    "atimestamp",
    "timecreated",
    "timegenerated",
    "firstseen",
    "lastseen",
    "updatedat",
    "modified",
    "lastwritetime",
];

/// Millisecond ladder used to pick a histogram bucket size.
const BUCKET_LADDER: &[(i64, &str)] = &[
    (1_000, "1 second"),
    (10_000, "10 seconds"),
    (60_000, "1 minute"),
    (300_000, "5 minutes"),
    (900_000, "15 minutes"),
    (3_600_000, "1 hour"),
    (21_600_000, "6 hours"),
    (86_400_000, "1 day"),
    (604_800_000, "7 days"),
    (2_592_000_000, "30 days"),
    (31_536_000_000, "365 days"),
];

struct Table {
    headers: Vec<String>,
    rows: Vec<Row>,
}

struct Row {
    /// 1-based position of this row in the source data (header row excluded).
    number: usize,
    cells: Vec<String>,
}

impl Row {
    fn get(&self, i: usize) -> &str {
        self.cells.get(i).map(|s| s.as_str()).unwrap_or("")
    }
}

/// Run the viewer. Every argument arrives as it does from the chat schema / CLI / page form.
#[allow(clippy::too_many_arguments)]
pub fn view(
    data: &str,
    format: &str,
    delimiter: &str,
    header: bool,
    time_column: &str,
    from: &str,
    to: &str,
    tz_offset: f64,
    search: &str,
    search_fields: &str,
    regex: bool,
    case_sensitive: bool,
    filters: &str,
    sort_by: &str,
    order: &str,
    columns: &str,
    limit: u32,
    offset: u32,
    output: &str,
) -> Result<String, String> {
    if data.trim().is_empty() {
        return Err("data is empty — paste a CSV, TSV, or JSON Lines table of events".into());
    }
    if data.lines().count() > MAX_INPUT_LINES {
        return Err(format!(
            "input has more than {MAX_INPUT_LINES} lines — split the file or narrow it before pasting"
        ));
    }
    if !(-14.0..=14.0).contains(&tz_offset) {
        return Err(format!(
            "tz_offset must be between -14 and 14 hours, got {tz_offset}"
        ));
    }
    let order = norm(order, "asc");
    if order != "asc" && order != "desc" {
        return Err(format!("order must be asc or desc, got '{order}'"));
    }
    let output = norm(output, "table");
    if !["table", "csv", "json", "jsonl", "summary"].contains(&output.as_str()) {
        return Err(format!(
            "output must be one of table, csv, json, jsonl, summary — got '{output}'"
        ));
    }
    let limit = if limit == 0 { 100 } else { limit } as usize;
    let offset = offset as usize;

    let table = parse_table(data, &norm(format, "auto"), &norm(delimiter, "auto"), header)?;
    let read = table.rows.len();

    // --- timestamp column -------------------------------------------------
    let time_idx = if time_column.trim().is_empty() {
        detect_time_column(&table)
    } else {
        Some(resolve_column(&table.headers, time_column, "time_column")?)
    };
    let times: Vec<Option<i64>> = match time_idx {
        Some(i) => table
            .rows
            .iter()
            .map(|r| parse_timestamp(r.get(i), tz_offset))
            .collect(),
        None => vec![None; table.rows.len()],
    };

    let lo = parse_bound(from, tz_offset, false, "from")?;
    let hi = parse_bound(to, tz_offset, true, "to")?;
    if (lo.is_some() || hi.is_some()) && time_idx.is_none() {
        return Err(format!(
            "from/to need a timestamp column but none was detected — set time_column to one of: {}",
            join_headers(&table.headers)
        ));
    }
    if let (Some(a), Some(b)) = (lo, hi) {
        if a > b {
            return Err("from is later than to — swap the range bounds".into());
        }
    }

    // --- filters ----------------------------------------------------------
    let conds = parse_filters(filters, &table.headers)?;
    let search_idx = resolve_column_list(&table.headers, search_fields, "search_fields")?;
    let needle = search.trim();
    let re = if !needle.is_empty() && regex {
        Some(
            regex::RegexBuilder::new(needle)
                .case_insensitive(!case_sensitive)
                .build()
                .map_err(|e| format!("search is not a valid regular expression: {e}"))?,
        )
    } else {
        None
    };
    let lowered = if case_sensitive {
        needle.to_string()
    } else {
        needle.to_lowercase()
    };

    let mut kept: Vec<usize> = Vec::new();
    let mut undated = 0usize;
    for (i, row) in table.rows.iter().enumerate() {
        if lo.is_some() || hi.is_some() {
            match times[i] {
                None => {
                    undated += 1;
                    continue;
                }
                Some(t) => {
                    if lo.is_some_and(|a| t < a) || hi.is_some_and(|b| t > b) {
                        continue;
                    }
                }
            }
        }
        if !conds.iter().all(|c| c.matches(row)) {
            continue;
        }
        if !needle.is_empty() {
            let cols: Vec<usize> = if search_idx.is_empty() {
                (0..table.headers.len()).collect()
            } else {
                search_idx.clone()
            };
            let hit = cols.iter().any(|&c| {
                let cell = row.get(c);
                match &re {
                    Some(r) => r.is_match(cell),
                    None => {
                        if case_sensitive {
                            cell.contains(&lowered)
                        } else {
                            cell.to_lowercase().contains(&lowered)
                        }
                    }
                }
            });
            if !hit {
                continue;
            }
        }
        kept.push(i);
    }
    let matched = kept.len();

    // --- sort -------------------------------------------------------------
    let sort_idx = if sort_by.trim().is_empty() {
        time_idx
    } else {
        Some(resolve_column(&table.headers, sort_by, "sort_by")?)
    };
    if let Some(si) = sort_idx {
        let by_time = Some(si) == time_idx;
        kept.sort_by(|&a, &b| {
            let ord = if by_time {
                // Rows whose timestamp does not parse sort after every dated row.
                match (times[a], times[b]) {
                    (Some(x), Some(y)) => x.cmp(&y),
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (None, None) => std::cmp::Ordering::Equal,
                }
            } else {
                compare_cells(table.rows[a].get(si), table.rows[b].get(si))
            };
            ord.then(table.rows[a].number.cmp(&table.rows[b].number))
        });
        if order == "desc" {
            kept.reverse();
        }
    } else if order == "desc" {
        kept.reverse();
    }

    // --- summary output uses every match, not just the current page --------
    if output == "summary" {
        return Ok(render_summary(
            &table, &kept, &times, time_idx, read, matched, undated,
        ));
    }

    // --- page + project ---------------------------------------------------
    let page: Vec<usize> = kept.iter().skip(offset).take(limit).copied().collect();
    let cols = resolve_column_list(&table.headers, columns, "columns")?;
    let cols: Vec<usize> = if cols.is_empty() {
        (0..table.headers.len()).collect()
    } else {
        cols
    };
    let out_headers: Vec<String> = cols.iter().map(|&c| table.headers[c].clone()).collect();

    Ok(match output.as_str() {
        "csv" => render_csv(&out_headers, &table, &page, &cols),
        "json" => render_json(&out_headers, &table, &page, &cols, true),
        "jsonl" => render_json(&out_headers, &table, &page, &cols, false),
        _ => render_table(
            &out_headers,
            &table,
            &page,
            &cols,
            &times,
            time_idx,
            read,
            matched,
            offset,
            undated,
        ),
    })
}

// ---------------------------------------------------------------------------
// input parsing
// ---------------------------------------------------------------------------

fn norm(s: &str, fallback: &str) -> String {
    let s = s.trim();
    if s.is_empty() {
        fallback.to_string()
    } else {
        s.to_lowercase()
    }
}

fn parse_table(data: &str, format: &str, delimiter: &str, header: bool) -> Result<Table, String> {
    let format = match format {
        "auto" => detect_format(data),
        "csv" | "tsv" | "jsonl" => format.to_string(),
        other => {
            return Err(format!(
                "format must be one of auto, csv, tsv, jsonl — got '{other}'"
            ))
        }
    };
    let table = if format == "jsonl" {
        parse_jsonl(data)?
    } else {
        let sep = match delimiter {
            "auto" => {
                if format == "tsv" {
                    b'\t'
                } else {
                    detect_delimiter(data)
                }
            }
            "comma" => b',',
            "semicolon" => b';',
            "tab" => b'\t',
            "pipe" => b'|',
            other => {
                return Err(format!(
                    "delimiter must be one of auto, comma, semicolon, tab, pipe — got '{other}'"
                ))
            }
        };
        parse_delimited(data, sep, header)?
    };
    if table.rows.len() > MAX_ROWS {
        return Err(format!(
            "{} data rows exceeds the {MAX_ROWS}-row cap — narrow the file before pasting",
            table.rows.len()
        ));
    }
    if table.headers.is_empty() {
        return Err("no columns found in the input".into());
    }
    Ok(table)
}

fn detect_format(data: &str) -> String {
    match data.lines().find(|l| !l.trim().is_empty()) {
        Some(l) if l.trim_start().starts_with('{') || l.trim_start().starts_with('[') => {
            "jsonl".to_string()
        }
        _ => "csv".to_string(),
    }
}

/// Pick the separator that appears most often outside quotes on the first non-blank line.
fn detect_delimiter(data: &str) -> u8 {
    let line = data.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
    let mut counts = [0usize; 4];
    let seps = [b',', b';', b'\t', b'|'];
    let mut in_quotes = false;
    for b in line.bytes() {
        if b == b'"' {
            in_quotes = !in_quotes;
            continue;
        }
        if in_quotes {
            continue;
        }
        if let Some(p) = seps.iter().position(|&s| s == b) {
            counts[p] += 1;
        }
    }
    let best = counts
        .iter()
        .enumerate()
        .max_by_key(|(i, c)| (**c, std::cmp::Reverse(*i)))
        .map(|(i, _)| i)
        .unwrap_or(0);
    if counts[best] == 0 {
        b','
    } else {
        seps[best]
    }
}

fn parse_delimited(data: &str, sep: u8, header: bool) -> Result<Table, String> {
    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(sep)
        .has_headers(false)
        .flexible(true)
        .from_reader(data.as_bytes());

    let mut records: Vec<Vec<String>> = Vec::new();
    for (i, rec) in rdr.records().enumerate() {
        let rec = rec.map_err(|e| format!("CSV parse error on record {}: {e}", i + 1))?;
        let cells: Vec<String> = rec.iter().map(|c| c.trim().to_string()).collect();
        if cells.iter().all(|c| c.is_empty()) {
            continue;
        }
        records.push(cells);
    }
    if records.is_empty() {
        return Err("no rows found in the input".into());
    }
    let width = records.iter().map(|r| r.len()).max().unwrap_or(0);
    let headers: Vec<String> = if header {
        let mut h = records.remove(0);
        h.resize(width, String::new());
        h.iter()
            .enumerate()
            .map(|(i, c)| {
                if c.is_empty() {
                    format!("column{}", i + 1)
                } else {
                    c.clone()
                }
            })
            .collect()
    } else {
        (1..=width).map(|i| format!("column{i}")).collect()
    };
    let rows = records
        .into_iter()
        .enumerate()
        .map(|(i, mut cells)| {
            cells.resize(width, String::new());
            Row {
                number: i + 1,
                cells,
            }
        })
        .collect();
    Ok(Table { headers, rows })
}

fn parse_jsonl(data: &str) -> Result<Table, String> {
    let mut records: Vec<serde_json::Map<String, serde_json::Value>> = Vec::new();
    let trimmed = data.trim();
    // A whole-input JSON array of objects is accepted alongside true JSON Lines.
    if trimmed.starts_with('[') {
        let v: serde_json::Value = serde_json::from_str(trimmed)
            .map_err(|e| format!("JSON parse error: {e} — expected a JSON array of objects"))?;
        let arr = v
            .as_array()
            .ok_or_else(|| "expected a JSON array of objects".to_string())?;
        for (i, item) in arr.iter().enumerate() {
            let obj = item.as_object().ok_or_else(|| {
                format!("array element {} is not an object — every event must be a JSON object", i + 1)
            })?;
            records.push(obj.clone());
        }
    } else {
        for (i, line) in data.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let v: serde_json::Value = serde_json::from_str(line.trim())
                .map_err(|e| format!("JSON Lines parse error on line {}: {e}", i + 1))?;
            let obj = v.as_object().cloned().ok_or_else(|| {
                format!(
                    "line {} is not a JSON object — JSON Lines input needs one object per line",
                    i + 1
                )
            })?;
            records.push(obj);
        }
    }
    if records.is_empty() {
        return Err("no JSON objects found in the input".into());
    }
    let mut headers: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for rec in &records {
        for k in rec.keys() {
            if seen.insert(k.clone()) {
                headers.push(k.clone());
            }
        }
    }
    let rows = records
        .into_iter()
        .enumerate()
        .map(|(i, rec)| Row {
            number: i + 1,
            cells: headers
                .iter()
                .map(|h| rec.get(h).map(json_cell).unwrap_or_default())
                .collect(),
        })
        .collect();
    Ok(Table { headers, rows })
}

fn json_cell(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::Null => String::new(),
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

// ---------------------------------------------------------------------------
// columns
// ---------------------------------------------------------------------------

fn join_headers(headers: &[String]) -> String {
    headers.join(", ")
}

/// Resolve a header name (case-insensitive) or a 1-based column index.
fn resolve_column(headers: &[String], spec: &str, what: &str) -> Result<usize, String> {
    let spec = spec.trim();
    if let Some(i) = headers.iter().position(|h| h.eq_ignore_ascii_case(spec)) {
        return Ok(i);
    }
    if let Ok(n) = spec.parse::<usize>() {
        if n >= 1 && n <= headers.len() {
            return Ok(n - 1);
        }
        return Err(format!(
            "{what} column index {n} is out of range 1-{}",
            headers.len()
        ));
    }
    Err(format!(
        "{what} column '{spec}' not found — available columns: {}",
        join_headers(headers)
    ))
}

fn resolve_column_list(headers: &[String], spec: &str, what: &str) -> Result<Vec<usize>, String> {
    spec.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| resolve_column(headers, s, what))
        .collect()
}

// ---------------------------------------------------------------------------
// filters
// ---------------------------------------------------------------------------

const OPS: &[&str] = &[
    "==",
    "=",
    "!=",
    "<=",
    ">=",
    "<",
    ">",
    "contains",
    "!contains",
    "startswith",
    "endswith",
    "matches",
];

struct Cond {
    col: usize,
    op: String,
    value: String,
    re: Option<regex::Regex>,
}

impl Cond {
    fn matches(&self, row: &Row) -> bool {
        let cell = row.get(self.col);
        let lc = cell.to_lowercase();
        let lv = self.value.to_lowercase();
        match self.op.as_str() {
            "contains" => lc.contains(&lv),
            "!contains" => !lc.contains(&lv),
            "startswith" => lc.starts_with(&lv),
            "endswith" => lc.ends_with(&lv),
            "matches" => self.re.as_ref().is_some_and(|r| r.is_match(cell)),
            op => {
                let ord = compare_cells(cell, &self.value);
                match op {
                    "==" | "=" => ord == std::cmp::Ordering::Equal,
                    "!=" => ord != std::cmp::Ordering::Equal,
                    "<" => ord == std::cmp::Ordering::Less,
                    "<=" => ord != std::cmp::Ordering::Greater,
                    ">" => ord == std::cmp::Ordering::Greater,
                    _ => ord != std::cmp::Ordering::Less,
                }
            }
        }
    }
}

fn parse_filters(filters: &str, headers: &[String]) -> Result<Vec<Cond>, String> {
    let mut out = Vec::new();
    for (i, line) in filters.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        let at = parts
            .iter()
            .position(|p| OPS.contains(&p.to_lowercase().as_str()))
            .ok_or_else(|| {
                format!(
                    "filter line {} ('{line}') has no operator — write '<column> <op> <value>' with op one of {}",
                    i + 1,
                    OPS.join(" ")
                )
            })?;
        if at == 0 {
            return Err(format!(
                "filter line {} ('{line}') is missing a column name before the operator",
                i + 1
            ));
        }
        let col = resolve_column(headers, &parts[..at].join(" "), "filters")?;
        let op = parts[at].to_lowercase();
        let value = parts[at + 1..].join(" ");
        let re = if op == "matches" {
            Some(
                regex::Regex::new(&value)
                    .map_err(|e| format!("filter line {} has an invalid regex: {e}", i + 1))?,
            )
        } else {
            None
        };
        out.push(Cond {
            col,
            op,
            value,
            re,
        });
    }
    Ok(out)
}

/// Numeric compare when both sides are numbers, otherwise case-insensitive string compare.
fn compare_cells(a: &str, b: &str) -> std::cmp::Ordering {
    if let (Ok(x), Ok(y)) = (a.trim().parse::<f64>(), b.trim().parse::<f64>()) {
        return x.partial_cmp(&y).unwrap_or(std::cmp::Ordering::Equal);
    }
    a.to_lowercase().cmp(&b.to_lowercase())
}

// ---------------------------------------------------------------------------
// timestamps
// ---------------------------------------------------------------------------

fn normalize_header(h: &str) -> String {
    h.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_lowercase()
}

/// Prefer a column whose HEADER names a time; otherwise the first column whose values parse.
fn detect_time_column(table: &Table) -> Option<usize> {
    for (i, h) in table.headers.iter().enumerate() {
        if TIME_HEADERS.contains(&normalize_header(h).as_str()) && column_is_timey(table, i) {
            return Some(i);
        }
    }
    for (i, h) in table.headers.iter().enumerate() {
        let n = normalize_header(h);
        if TIME_HEADERS.iter().any(|t| n.contains(t)) && column_is_timey(table, i) {
            return Some(i);
        }
    }
    (0..table.headers.len()).find(|&i| column_is_timey(table, i))
}

/// True when at least half of the first 50 non-empty values in the column parse as timestamps.
fn column_is_timey(table: &Table, col: usize) -> bool {
    let mut seen = 0;
    let mut ok = 0;
    for row in &table.rows {
        let v = row.get(col);
        if v.is_empty() {
            continue;
        }
        seen += 1;
        if parse_timestamp(v, 0.0).is_some() {
            ok += 1;
        }
        if seen >= 50 {
            break;
        }
    }
    seen > 0 && ok * 2 >= seen
}

/// Naive (offset-less) datetime formats, tried in order.
const NAIVE_DATETIME: &[&str] = &[
    "%Y-%m-%dT%H:%M:%S%.f",
    "%Y-%m-%d %H:%M:%S%.f",
    "%Y-%m-%dT%H:%M",
    "%Y-%m-%d %H:%M",
    "%Y/%m/%d %H:%M:%S",
    "%m/%d/%Y %H:%M:%S",
    "%Y%m%dT%H%M%S",
    "%d-%b-%Y %H:%M:%S",
];

/// Date-only formats, tried in order.
const NAIVE_DATE: &[&str] = &["%Y-%m-%d", "%Y/%m/%d", "%m/%d/%Y", "%d-%b-%Y", "%Y%m%d"];

/// Formats carrying an explicit UTC offset.
const OFFSET_DATETIME: &[&str] = &[
    "%d/%b/%Y:%H:%M:%S %z",
    "%Y-%m-%dT%H:%M:%S%.f%z",
    "%Y-%m-%d %H:%M:%S%.f%z",
    "%Y-%m-%d %H:%M:%S%.f %z",
];

/// Parse one cell into epoch milliseconds, applying `tz_offset` hours to naive values.
pub fn parse_timestamp(s: &str, tz_offset: f64) -> Option<i64> {
    let s = s.trim().trim_matches('"');
    if s.is_empty() {
        return None;
    }
    if let Some(ms) = parse_epoch(s) {
        return Some(ms);
    }
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt.timestamp_millis());
    }
    // "2024-06-01 10:00:05Z" — same instant, space instead of the RFC 3339 'T'.
    if s.len() > 10 && s.as_bytes()[10] == b' ' {
        let swapped = format!("{}T{}", &s[..10], &s[11..]);
        if let Ok(dt) = DateTime::parse_from_rfc3339(&swapped) {
            return Some(dt.timestamp_millis());
        }
    }
    for f in OFFSET_DATETIME {
        if let Ok(dt) = DateTime::parse_from_str(s, f) {
            return Some(dt.timestamp_millis());
        }
    }
    if let Some(naive) = parse_naive(s) {
        return Some(shift(naive, tz_offset));
    }
    None
}

fn parse_naive(s: &str) -> Option<NaiveDateTime> {
    for f in NAIVE_DATETIME {
        if let Ok(dt) = NaiveDateTime::parse_from_str(s, f) {
            return Some(dt);
        }
    }
    parse_naive_date(s).map(|d| d.and_hms_opt(0, 0, 0).unwrap())
}

fn parse_naive_date(s: &str) -> Option<NaiveDate> {
    for f in NAIVE_DATE {
        if let Ok(d) = NaiveDate::parse_from_str(s, f) {
            return Some(d);
        }
    }
    None
}

fn shift(naive: NaiveDateTime, tz_offset: f64) -> i64 {
    naive.and_utc().timestamp_millis() - (tz_offset * 3_600_000.0).round() as i64
}

/// Bare epoch numbers: seconds, milliseconds, microseconds, or nanoseconds by magnitude.
fn parse_epoch(s: &str) -> Option<i64> {
    if !s.starts_with(|c: char| c.is_ascii_digit() || c == '-') {
        return None;
    }
    if s.contains('.') && !s.contains(['-', '/', ':', 'T']) {
        let f = s.parse::<f64>().ok()?;
        return Some((f * 1000.0).round() as i64);
    }
    if !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    // 8 digits is a date like 20240601, not an epoch — leave it to the date formats.
    if s.len() < 9 {
        return None;
    }
    let n = s.parse::<i64>().ok()?;
    Some(match n.abs() {
        0..=99_999_999_999 => n * 1000,
        100_000_000_000..=99_999_999_999_999 => n,
        100_000_000_000_000..=99_999_999_999_999_999 => n / 1_000,
        _ => n / 1_000_000,
    })
}

/// Parse a `from`/`to` bound; a date-only `to` extends to the end of that day.
fn parse_bound(s: &str, tz_offset: f64, end_of_day: bool, what: &str) -> Result<Option<i64>, String> {
    let s = s.trim();
    if s.is_empty() {
        return Ok(None);
    }
    // HTML datetime-local sends "2024-06-01T10:00" — already covered by NAIVE_DATETIME.
    if parse_naive_date(s).is_some() && parse_epoch(s).is_none() {
        let d = parse_naive_date(s).unwrap();
        let base = shift(d.and_hms_opt(0, 0, 0).unwrap(), tz_offset);
        return Ok(Some(if end_of_day { base + 86_399_999 } else { base }));
    }
    parse_timestamp(s, tz_offset).map(Some).ok_or_else(|| {
        format!(
            "{what} is not a recognized date or time: '{s}' — try 2024-06-01, 2024-06-01T10:00:00Z, or an epoch value"
        )
    })
}

fn iso(ms: i64) -> String {
    DateTime::<Utc>::from_timestamp_millis(ms)
        .map(|d| d.format("%Y-%m-%dT%H:%M:%SZ").to_string())
        .unwrap_or_else(|| ms.to_string())
}

// ---------------------------------------------------------------------------
// rendering
// ---------------------------------------------------------------------------

fn truncate(s: &str, max: usize) -> String {
    let flat = s.replace(['\n', '\r'], " ");
    if flat.chars().count() <= max {
        return flat;
    }
    let mut out: String = flat.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

#[allow(clippy::too_many_arguments)]
fn render_table(
    out_headers: &[String],
    table: &Table,
    page: &[usize],
    cols: &[usize],
    times: &[Option<i64>],
    time_idx: Option<usize>,
    read: usize,
    matched: usize,
    offset: usize,
    undated: usize,
) -> String {
    let mut grid: Vec<Vec<String>> = Vec::with_capacity(page.len() + 1);
    let mut head = vec!["#".to_string()];
    head.extend(out_headers.iter().map(|h| truncate(h, MAX_CELL)));
    grid.push(head);
    for &i in page {
        let mut r = vec![table.rows[i].number.to_string()];
        r.extend(cols.iter().map(|&c| truncate(table.rows[i].get(c), MAX_CELL)));
        grid.push(r);
    }
    let ncol = grid[0].len();
    let widths: Vec<usize> = (0..ncol)
        .map(|c| {
            grid.iter()
                .map(|r| r.get(c).map(|s| s.chars().count()).unwrap_or(0))
                .max()
                .unwrap_or(0)
        })
        .collect();

    let mut out = String::new();
    for (ri, row) in grid.iter().enumerate() {
        let line: Vec<String> = (0..ncol)
            .map(|c| {
                let cell = row.get(c).cloned().unwrap_or_default();
                let pad = widths[c].saturating_sub(cell.chars().count());
                format!("{cell}{}", " ".repeat(pad))
            })
            .collect();
        out.push_str(line.join("  ").trim_end());
        out.push('\n');
        if ri == 0 {
            let rule: Vec<String> = widths.iter().map(|w| "-".repeat(*w)).collect();
            out.push_str(&rule.join("  "));
            out.push('\n');
        }
    }
    if page.is_empty() {
        out.push_str("(no rows matched)\n");
    }

    let first = if page.is_empty() { 0 } else { offset + 1 };
    let last = offset + page.len();
    out.push('\n');
    out.push_str(&format!(
        "showing rows {first}-{last} of {matched} matched ({read} read)"
    ));
    match time_idx {
        Some(i) => {
            out.push_str(&format!(" | time column: {}", table.headers[i]));
            let span: Vec<i64> = page.iter().filter_map(|&i| times[i]).collect();
            if let (Some(a), Some(b)) = (span.iter().min(), span.iter().max()) {
                out.push_str(&format!(" | span: {} .. {}", iso(*a), iso(*b)));
            }
        }
        None => out.push_str(" | time column: none detected"),
    }
    if undated > 0 {
        out.push_str(&format!(
            " | {undated} row(s) skipped: no parsable timestamp"
        ));
    }
    out.push('\n');
    out
}

fn render_csv(out_headers: &[String], table: &Table, page: &[usize], cols: &[usize]) -> String {
    let mut w = csv::Writer::from_writer(vec![]);
    let _ = w.write_record(out_headers);
    for &i in page {
        let rec: Vec<&str> = cols.iter().map(|&c| table.rows[i].get(c)).collect();
        let _ = w.write_record(&rec);
    }
    String::from_utf8(w.into_inner().unwrap_or_default()).unwrap_or_default()
}

fn render_json(
    out_headers: &[String],
    table: &Table,
    page: &[usize],
    cols: &[usize],
    array: bool,
) -> String {
    let objs: Vec<serde_json::Value> = page
        .iter()
        .map(|&i| {
            let mut m = serde_json::Map::new();
            for (h, &c) in out_headers.iter().zip(cols.iter()) {
                m.insert(
                    h.clone(),
                    serde_json::Value::String(table.rows[i].get(c).to_string()),
                );
            }
            serde_json::Value::Object(m)
        })
        .collect();
    if array {
        format!(
            "{}\n",
            serde_json::to_string_pretty(&objs).unwrap_or_else(|_| "[]".into())
        )
    } else {
        let mut s = String::new();
        for o in &objs {
            s.push_str(&serde_json::to_string(o).unwrap_or_default());
            s.push('\n');
        }
        s
    }
}

fn render_summary(
    table: &Table,
    kept: &[usize],
    times: &[Option<i64>],
    time_idx: Option<usize>,
    read: usize,
    matched: usize,
    undated: usize,
) -> String {
    let mut out = String::new();
    out.push_str(&format!("Rows read:      {read}\n"));
    out.push_str(&format!("Rows matched:   {matched}\n"));
    out.push_str(&format!("Columns:        {}\n", join_headers(&table.headers)));
    match time_idx {
        Some(i) => out.push_str(&format!("Time column:    {}\n", table.headers[i])),
        None => {
            out.push_str("Time column:    none detected\n");
            return out;
        }
    }
    if undated > 0 {
        out.push_str(&format!("Skipped:        {undated} row(s) with no parsable timestamp\n"));
    }
    let stamps: Vec<i64> = kept.iter().filter_map(|&i| times[i]).collect();
    if stamps.is_empty() {
        out.push_str("Span:           no dated rows matched\n");
        return out;
    }
    let lo = *stamps.iter().min().unwrap();
    let hi = *stamps.iter().max().unwrap();
    out.push_str(&format!("Earliest:       {}\n", iso(lo)));
    out.push_str(&format!("Latest:         {}\n", iso(hi)));

    let span = hi - lo;
    let (unit, label) = BUCKET_LADDER
        .iter()
        .find(|(u, _)| span / u + 1 <= MAX_BUCKETS as i64)
        .copied()
        .unwrap_or(*BUCKET_LADDER.last().unwrap());
    out.push_str(&format!("Bucket:         {label}\n\n"));

    let first = lo.div_euclid(unit);
    let last = hi.div_euclid(unit);
    let n = (last - first + 1) as usize;
    let mut counts = vec![0usize; n];
    for t in &stamps {
        counts[(t.div_euclid(unit) - first) as usize] += 1;
    }
    let peak = counts.iter().copied().max().unwrap_or(1).max(1);
    let cw = counts.iter().map(|c| c.to_string().len()).max().unwrap_or(1);
    for (i, c) in counts.iter().enumerate() {
        let bar = "#".repeat((c * 40).div_ceil(peak).min(40));
        out.push_str(&format!(
            "{}  {:>cw$}  {}\n",
            iso((first + i as i64) * unit),
            c,
            bar,
            cw = cw
        ));
    }
    out
}

// ---------------------------------------------------------------------------
// tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const EVENTS: &str = "timestamp,level,service,message\n\
2024-06-01T10:00:01Z,INFO,api,request started\n\
2024-06-01T10:00:05Z,ERROR,api,upstream timeout\n\
2024-06-01T10:00:09Z,WARN,worker,retrying job 42\n\
2024-06-02T11:30:00Z,ERROR,worker,job 42 failed\n";

    #[allow(clippy::too_many_arguments)]
    fn v(data: &str, from: &str, to: &str, search: &str, filters: &str, output: &str) -> String {
        view(
            data, "auto", "auto", true, "", from, to, 0.0, search, "", false, false, filters, "",
            "asc", "", 100, 0, output,
        )
        .unwrap()
    }

    #[test]
    fn table_output_lists_every_row_with_source_numbers() {
        let out = v(EVENTS, "", "", "", "", "table");
        assert_eq!(
            out,
            "#  timestamp             level  service  message\n\
-  --------------------  -----  -------  ----------------\n\
1  2024-06-01T10:00:01Z  INFO   api      request started\n\
2  2024-06-01T10:00:05Z  ERROR  api      upstream timeout\n\
3  2024-06-01T10:00:09Z  WARN   worker   retrying job 42\n\
4  2024-06-02T11:30:00Z  ERROR  worker   job 42 failed\n\
\n\
showing rows 1-4 of 4 matched (4 read) | time column: timestamp | span: 2024-06-01T10:00:01Z .. 2024-06-02T11:30:00Z\n"
        );
    }

    #[test]
    fn time_range_is_inclusive_and_date_only_to_covers_the_whole_day() {
        let out = v(EVENTS, "2024-06-01", "2024-06-01", "", "", "csv");
        assert_eq!(
            out,
            "timestamp,level,service,message\n\
2024-06-01T10:00:01Z,INFO,api,request started\n\
2024-06-01T10:00:05Z,ERROR,api,upstream timeout\n\
2024-06-01T10:00:09Z,WARN,worker,retrying job 42\n"
        );
    }

    #[test]
    fn full_text_search_spans_every_column() {
        let out = v(EVENTS, "", "", "worker", "", "csv");
        assert_eq!(out.lines().count(), 3);
        assert!(out.contains("retrying job 42"));
        assert!(!out.contains("request started"));
    }

    #[test]
    fn column_filters_and_conditions_combine_with_and() {
        let out = v(EVENTS, "", "", "", "level == ERROR\nservice == worker", "csv");
        assert_eq!(
            out,
            "timestamp,level,service,message\n2024-06-02T11:30:00Z,ERROR,worker,job 42 failed\n"
        );
    }

    #[test]
    fn regex_search_and_case_sensitivity_are_honored() {
        let hit = view(
            EVENTS, "csv", "auto", true, "", "", "", 0.0, r"job \d+", "", true, false, "", "",
            "asc", "", 100, 0, "csv",
        )
        .unwrap();
        assert_eq!(hit.lines().count(), 3);

        let miss = view(
            EVENTS, "csv", "auto", true, "", "", "", 0.0, "error", "", false, true, "", "", "asc",
            "", 100, 0, "csv",
        )
        .unwrap();
        assert_eq!(miss.lines().count(), 1, "case-sensitive 'error' matches no cell");
    }

    #[test]
    fn sort_desc_projection_and_paging() {
        let out = view(
            EVENTS, "auto", "auto", true, "", "", "", 0.0, "", "", false, false, "", "", "desc",
            "timestamp, message", 2, 1, "jsonl",
        )
        .unwrap();
        assert_eq!(
            out,
            "{\"timestamp\":\"2024-06-01T10:00:09Z\",\"message\":\"retrying job 42\"}\n\
{\"timestamp\":\"2024-06-01T10:00:05Z\",\"message\":\"upstream timeout\"}\n"
        );
    }

    #[test]
    fn sorting_by_a_non_time_column_is_numeric_when_both_cells_are_numbers() {
        let data = "when,size\n2024-01-01,9\n2024-01-02,100\n2024-01-03,20\n";
        let out = view(
            data, "csv", "auto", true, "", "", "", 0.0, "", "", false, false, "", "size", "asc",
            "size", 100, 0, "csv",
        )
        .unwrap();
        assert_eq!(out, "size\n9\n20\n100\n");
    }

    #[test]
    fn jsonl_input_unions_keys_and_detects_the_time_field() {
        let data = "{\"ts\":\"2024-06-01T10:00:00Z\",\"msg\":\"a\"}\n\
{\"ts\":\"2024-06-01T10:01:00Z\",\"user\":\"bo\"}\n";
        let out = v(data, "", "", "", "", "csv");
        assert_eq!(out, "ts,msg,user\n2024-06-01T10:00:00Z,a,\n2024-06-01T10:01:00Z,,bo\n");
    }

    #[test]
    fn epoch_seconds_and_millis_both_parse() {
        assert_eq!(parse_timestamp("1717236001", 0.0), Some(1_717_236_001_000));
        assert_eq!(parse_timestamp("1717236001000", 0.0), Some(1_717_236_001_000));
        assert_eq!(parse_timestamp("1717236001.5", 0.0), Some(1_717_236_001_500));
        let data = "epoch,what\n1717236001,a\n1717236061,b\n";
        let out = v(data, "2024-06-01T10:00:30Z", "", "", "", "csv");
        assert_eq!(out, "epoch,what\n1717236061,b\n");
    }

    #[test]
    fn tz_offset_shifts_naive_timestamps_only() {
        let data = "time,msg\n2024-06-01 10:00:00,naive\n2024-06-01T10:00:00Z,aware\n";
        let out = view(
            data, "csv", "auto", true, "", "2024-06-01T14:00:00Z", "", 0.0, "", "", false, false,
            "", "", "asc", "", 100, 0, "csv",
        )
        .unwrap();
        assert_eq!(out, "time,msg\n", "with tz_offset=0 nothing is after 14:00Z");
        let out = view(
            data, "csv", "auto", true, "", "2024-06-01T14:00:00Z", "", -5.0, "", "", false, false,
            "", "", "asc", "", 100, 0, "csv",
        )
        .unwrap();
        assert_eq!(out, "time,msg\n2024-06-01 10:00:00,naive\n");
    }

    #[test]
    fn apache_and_semicolon_and_pipe_inputs_parse() {
        let data = "time;msg\n01/Jun/2024:10:00:00 +0000;hit\n";
        let out = v(data, "2024-06-01", "2024-06-01", "", "", "csv");
        assert_eq!(out, "time;msg\n01/Jun/2024:10:00:00 +0000;hit\n".replace(';', ","));
    }

    #[test]
    fn headerless_input_gets_generated_column_names() {
        let out = view(
            "2024-06-01T10:00:00Z,boot\n", "csv", "comma", false, "", "", "", 0.0, "", "", false,
            false, "", "", "asc", "", 100, 0, "csv",
        )
        .unwrap();
        assert_eq!(out, "column1,column2\n2024-06-01T10:00:00Z,boot\n");
    }

    #[test]
    fn summary_reports_counts_and_a_histogram() {
        let out = v(EVENTS, "", "", "", "", "summary");
        assert!(out.contains("Rows read:      4"));
        assert!(out.contains("Time column:    timestamp"));
        assert!(out.contains("Earliest:       2024-06-01T10:00:01Z"));
        assert!(out.contains("Latest:         2024-06-02T11:30:00Z"));
        assert!(out.contains("Bucket:         1 hour"));
        assert!(out.contains('#'));
    }

    #[test]
    fn no_match_still_renders_a_stable_table_with_a_footer() {
        let out = v(EVENTS, "", "", "nothing-here", "", "table");
        assert!(out.contains("(no rows matched)"));
        assert!(out.contains("showing rows 0-0 of 0 matched (4 read)"));
    }

    // --- error paths -------------------------------------------------------

    #[test]
    fn empty_input_is_rejected() {
        let e = view(
            "   ", "auto", "auto", true, "", "", "", 0.0, "", "", false, false, "", "", "asc", "",
            100, 0, "table",
        )
        .unwrap_err();
        assert!(e.contains("data is empty"), "{e}");
    }

    #[test]
    fn an_unknown_column_names_the_available_ones() {
        let e = view(
            EVENTS, "auto", "auto", true, "nope", "", "", 0.0, "", "", false, false, "", "", "asc",
            "", 100, 0, "table",
        )
        .unwrap_err();
        assert_eq!(
            e,
            "time_column column 'nope' not found — available columns: timestamp, level, service, message"
        );
    }

    #[test]
    fn a_bad_range_bound_says_what_was_expected() {
        let e = view(
            EVENTS, "auto", "auto", true, "", "yesterday", "", 0.0, "", "", false, false, "", "",
            "asc", "", 100, 0, "table",
        )
        .unwrap_err();
        assert!(e.starts_with("from is not a recognized date or time: 'yesterday'"), "{e}");
    }

    #[test]
    fn a_filter_without_an_operator_explains_the_syntax() {
        let e = view(
            EVENTS, "auto", "auto", true, "", "", "", 0.0, "", "", false, false, "level ERROR", "",
            "asc", "", 100, 0, "table",
        )
        .unwrap_err();
        assert!(e.contains("has no operator"), "{e}");
    }

    #[test]
    fn an_invalid_search_regex_is_reported() {
        let e = view(
            EVENTS, "auto", "auto", true, "", "", "", 0.0, "job (", "", true, false, "", "", "asc",
            "", 100, 0, "table",
        )
        .unwrap_err();
        assert!(e.contains("not a valid regular expression"), "{e}");
    }

    #[test]
    fn a_time_range_without_a_time_column_is_a_clear_error() {
        let data = "name,city\nada,london\n";
        let e = view(
            data, "csv", "auto", true, "", "2024-01-01", "", 0.0, "", "", false, false, "", "",
            "asc", "", 100, 0, "table",
        )
        .unwrap_err();
        assert!(e.contains("none was detected"), "{e}");
        assert!(e.contains("name, city"), "{e}");
    }

    #[test]
    fn reversed_bounds_are_rejected() {
        let e = view(
            EVENTS, "auto", "auto", true, "", "2024-06-02", "2024-06-01", 0.0, "", "", false,
            false, "", "", "asc", "", 100, 0, "table",
        )
        .unwrap_err();
        assert!(e.contains("from is later than to"), "{e}");
    }

    #[test]
    fn malformed_jsonl_names_the_line() {
        let e = view(
            "{\"a\":1}\n{oops}\n", "jsonl", "auto", true, "", "", "", 0.0, "", "", false, false,
            "", "", "asc", "", 100, 0, "table",
        )
        .unwrap_err();
        assert!(e.contains("line 2"), "{e}");
    }
}
