//! log-to-metrics core — turn structured log lines into RED-style metrics:
//! counts, throughput rates, error rates and exact latency percentiles, grouped
//! by any field. No wafer/wasm-bindgen deps; shared by the chat skill block, the
//! CLI and the web page.
//!
//! Pipeline, in order:
//!
//! 1. **Detect + parse** each line as JSON/NDJSON, logfmt (`key=value`) or CSV
//!    (header row + rows). Nested JSON objects flatten to dotted paths, so
//!    `{"http":{"status":500}}` is reachable as `http.status`.
//! 2. **Extract** only the four fields the aggregation needs from every row —
//!    the group-by fields, the numeric value field, the timestamp field and the
//!    error field — so nothing else is retained in memory.
//! 3. **Accumulate** per group: line count, error count and every parsed value.
//! 4. **Reduce**: percent of total, rate over the log's own time span, error
//!    percent, min/avg/max/sum and exact percentiles (linear interpolation or
//!    nearest-rank) of the value field.
//! 5. **Rank, truncate and render** as an aligned Markdown table, JSON, CSV, or
//!    Prometheus text exposition.
//!
//! Everything is deterministic: one pass in input order, ties broken by first
//! appearance, percentiles computed exactly from the values in the batch (no
//! sketching, no sampling), so the same input always produces the same output on
//! every surface. There is no persisted state between runs.

use std::collections::HashMap;

use serde_json::{json, Map, Value};

/// Largest input accepted, in Unicode characters.
pub const MAX_CHARS: usize = 2_000_000;
/// Largest number of input lines accepted.
pub const MAX_LINES: usize = 200_000;
/// Largest number of distinct groups accepted before the run is refused.
pub const MAX_GROUPS: usize = 50_000;
/// Largest number of fields `group_by` may name.
pub const MAX_GROUP_FIELDS: usize = 5;
/// Largest number of percentiles that can be requested at once.
pub const MAX_PERCENTILES: usize = 10;
/// Value used for a row whose group field is absent (or JSON `null`).
pub const MISSING: &str = "(missing)";
/// Group label used when `group_by` is blank (everything in one bucket).
pub const ALL: &str = "(all)";
/// Group label of the rolled-up remainder row.
pub const OTHER: &str = "(other)";

/// Error-matching rules applied when `error_values` is left blank.
const DEFAULT_ERROR_VALUES: &[&str] = &[
    "5*", "error", "err", "fatal", "critical", "crit", "panic", "emerg", "alert",
];

/// Field names probed, in order, when `time_field` is blank.
const TIME_FIELD_CANDIDATES: &[&str] = &[
    "timestamp",
    "@timestamp",
    "time",
    "ts",
    "datetime",
    "date",
    "event_time",
    "eventtime",
    "_time",
];

// ---------------------------------------------------------------------------
// Options.
// ---------------------------------------------------------------------------

/// Input log format.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Format {
    /// One JSON object per line (NDJSON).
    Json,
    /// `key=value` pairs, values optionally double-quoted.
    Logfmt,
    /// Header row plus delimited rows (comma, tab or semicolon).
    Csv,
}

impl Format {
    fn name(self) -> &'static str {
        match self {
            Format::Json => "json",
            Format::Logfmt => "logfmt",
            Format::Csv => "csv",
        }
    }
}

/// How a percentile is computed from the sorted values.
#[derive(Clone, Copy, PartialEq, Eq)]
enum PctMethod {
    /// Linear interpolation between order statistics (numpy/R type 7).
    Linear,
    /// Nearest-rank: the smallest value at or above the requested rank.
    Nearest,
}

/// Rate denominator.
#[derive(Clone, Copy, PartialEq, Eq)]
enum RateUnit {
    Second,
    Minute,
    Hour,
}

impl RateUnit {
    fn seconds(self) -> f64 {
        match self {
            RateUnit::Second => 1.0,
            RateUnit::Minute => 60.0,
            RateUnit::Hour => 3600.0,
        }
    }
    fn column(self) -> &'static str {
        match self {
            RateUnit::Second => "rate/s",
            RateUnit::Minute => "rate/min",
            RateUnit::Hour => "rate/h",
        }
    }
    fn name(self) -> &'static str {
        match self {
            RateUnit::Second => "second",
            RateUnit::Minute => "minute",
            RateUnit::Hour => "hour",
        }
    }
    fn metric_suffix(self) -> &'static str {
        match self {
            RateUnit::Second => "per_second",
            RateUnit::Minute => "per_minute",
            RateUnit::Hour => "per_hour",
        }
    }
}

/// One error-matching rule parsed out of `error_values`.
enum ErrorRule {
    /// Case-insensitive exact match.
    Eq(String),
    /// Case-insensitive prefix match (`5*`).
    Prefix(String),
    /// Numeric comparison (`>=500`).
    Cmp(&'static str, f64),
}

impl ErrorRule {
    fn matches(&self, raw: &str) -> bool {
        match self {
            ErrorRule::Eq(v) => raw.eq_ignore_ascii_case(v),
            ErrorRule::Prefix(p) => raw.to_ascii_lowercase().starts_with(p),
            ErrorRule::Cmp(op, n) => raw.trim().parse::<f64>().is_ok_and(|v| match *op {
                ">=" => v >= *n,
                ">" => v > *n,
                "<=" => v <= *n,
                "<" => v < *n,
                _ => (v - *n).abs() < f64::EPSILON,
            }),
        }
    }
}

// ---------------------------------------------------------------------------
// Public entry point.
// ---------------------------------------------------------------------------

/// Aggregate `data` into per-group metrics and render them.
///
/// Every parameter arrives as text (the CLI, chat and the page all pass
/// strings); blank means "use the default". Returns the rendered report, or a
/// human-readable error explaining what was expected.
#[allow(clippy::too_many_arguments)]
pub fn aggregate(
    data: &str,
    format: &str,
    group_by: &str,
    value_field: &str,
    percentiles: &str,
    percentile_method: &str,
    time_field: &str,
    rate_unit: &str,
    error_field: &str,
    error_values: &str,
    limit: u32,
    other: bool,
    sort: &str,
    output: &str,
    metric_prefix: &str,
) -> Result<String, String> {
    // ---- validate the knobs before touching the (potentially huge) input ----
    let output = norm(output, "table");
    if !matches!(output.as_str(), "table" | "json" | "csv" | "prometheus") {
        return Err(format!(
            "invalid output {output:?}: expected 'table', 'json', 'csv' or 'prometheus'"
        ));
    }
    let sort = norm(sort, "count");
    if !matches!(
        sort.as_str(),
        "count" | "group" | "sum" | "avg" | "max" | "errors" | "p_top"
    ) {
        return Err(format!(
            "invalid sort {sort:?}: expected 'count', 'group', 'sum', 'avg', 'max', 'errors' or 'p_top'"
        ));
    }
    let method = match norm(percentile_method, "linear").as_str() {
        "linear" => PctMethod::Linear,
        "nearest" => PctMethod::Nearest,
        m => {
            return Err(format!(
                "invalid percentile_method {m:?}: expected 'linear' or 'nearest'"
            ))
        }
    };
    if !(1..=1000).contains(&limit) {
        return Err(format!("invalid limit {limit}: expected 1-1000"));
    }
    let pcts = parse_percentiles(percentiles)?;
    let group_fields = parse_group_fields(group_by)?;
    let value_field = value_field.trim();
    let error_field = error_field.trim();
    let rules = parse_error_rules(error_values)?;
    let rate_unit_opt = match norm(rate_unit, "auto").as_str() {
        "auto" => None,
        "second" => Some(RateUnit::Second),
        "minute" => Some(RateUnit::Minute),
        "hour" => Some(RateUnit::Hour),
        u => {
            return Err(format!(
                "invalid rate_unit {u:?}: expected 'auto', 'second', 'minute' or 'hour'"
            ))
        }
    };
    let prefix = sanitize_metric(metric_prefix.trim(), "log");

    // ---- input caps ----
    if data.trim().is_empty() {
        return Err("no input: paste some log lines first".into());
    }
    if data.chars().count() > MAX_CHARS {
        return Err(format!(
            "input too large: {} characters, the maximum is {MAX_CHARS}",
            data.chars().count()
        ));
    }
    let lines: Vec<&str> = data.lines().collect();
    if lines.len() > MAX_LINES {
        return Err(format!(
            "too many lines: {}, the maximum is {MAX_LINES}",
            lines.len()
        ));
    }

    // ---- format ----
    let requested = norm(format, "auto");
    let fmt = match requested.as_str() {
        "auto" => detect_format(&lines),
        "json" | "ndjson" => Format::Json,
        "logfmt" => Format::Logfmt,
        "csv" | "tsv" => Format::Csv,
        f => {
            return Err(format!(
                "invalid format {f:?}: expected 'auto', 'json', 'logfmt' or 'csv'"
            ))
        }
    };

    // ---- parse + accumulate ----
    let mut csv_header: Option<(Vec<String>, char)> = None;
    let mut keys: HashMap<String, usize> = HashMap::new();
    let mut aggs: Vec<Agg> = Vec::new();
    let mut total_lines = 0usize;
    let mut parsed = 0usize;
    let mut unparsed = 0usize;
    let mut value_missing = 0usize;
    let mut value_unparsed = 0usize;
    let mut min_ts = f64::INFINITY;
    let mut max_ts = f64::NEG_INFINITY;
    let mut ts_seen = 0usize;
    let explicit_time = time_field.trim();
    let mut chosen_time: Option<String> = match explicit_time {
        "" | "none" => None,
        f => Some(f.to_string()),
    };
    let time_disabled = explicit_time.eq_ignore_ascii_case("none");

    for raw in &lines {
        if raw.trim().is_empty() {
            continue;
        }
        total_lines += 1;
        let fields = match fmt {
            Format::Json => parse_json_line(raw),
            Format::Logfmt => parse_logfmt_line(raw),
            Format::Csv => {
                let (header, delim) = match &csv_header {
                    Some(h) => h.clone(),
                    None => {
                        let delim = detect_delimiter(raw);
                        let header: Vec<String> = split_delimited(raw, delim)
                            .into_iter()
                            .map(|h| h.trim().to_string())
                            .collect();
                        csv_header = Some((header.clone(), delim));
                        // The header row itself is not a data row.
                        total_lines -= 1;
                        continue;
                    }
                };
                Some(zip_csv(&header, &split_delimited(raw, delim)))
            }
        };
        let Some(fields) = fields else {
            unparsed += 1;
            continue;
        };
        parsed += 1;

        // Timestamp (auto-detected from the first row that offers one).
        if !time_disabled {
            if chosen_time.is_none() {
                chosen_time = TIME_FIELD_CANDIDATES.iter().find_map(|cand| {
                    lookup(&fields, cand)
                        .and_then(parse_time)
                        .map(|_| cand.to_string())
                });
            }
            if let Some(tf) = &chosen_time {
                if let Some(t) = lookup(&fields, tf).and_then(parse_time) {
                    ts_seen += 1;
                    min_ts = min_ts.min(t);
                    max_ts = max_ts.max(t);
                }
            }
        }

        // Group key.
        let key: Vec<String> = if group_fields.is_empty() {
            vec![ALL.to_string()]
        } else {
            group_fields
                .iter()
                .map(|f| match lookup(&fields, f) {
                    Some(v) if !v.is_empty() => v.to_string(),
                    _ => MISSING.to_string(),
                })
                .collect()
        };
        let joined = key.join("\u{1}");
        let idx = match keys.get(&joined) {
            Some(i) => *i,
            None => {
                if aggs.len() >= MAX_GROUPS {
                    return Err(format!(
                        "too many distinct groups: over {MAX_GROUPS}. Group by a lower-cardinality field, or filter the log first."
                    ));
                }
                keys.insert(joined, aggs.len());
                aggs.push(Agg::new(key));
                aggs.len() - 1
            }
        };
        let agg = &mut aggs[idx];
        agg.count += 1;

        // Value field.
        if !value_field.is_empty() {
            match lookup(&fields, value_field) {
                None => value_missing += 1,
                Some(v) if v.trim().is_empty() => value_missing += 1,
                Some(v) => match parse_number(v) {
                    Some(n) => agg.values.push(n),
                    None => value_unparsed += 1,
                },
            }
        }

        // Error field.
        if !error_field.is_empty() {
            if let Some(v) = lookup(&fields, error_field) {
                if rules.iter().any(|r| r.matches(v)) {
                    agg.errors += 1;
                }
            }
        }
    }

    if parsed == 0 {
        return Err(format!(
            "no {} records found in the input ({total_lines} non-blank line(s) could not be parsed). Set format explicitly, or check the log really is JSON, logfmt or CSV.",
            fmt.name()
        ));
    }

    // ---- span + rate unit ----
    let span = if ts_seen >= 2 && max_ts > min_ts {
        Some(max_ts - min_ts)
    } else {
        None
    };
    // "auto": one step coarser than the span, so the number stays readable —
    // a log covering under a minute reads per second, under an hour per
    // minute, anything longer per hour.
    let unit = rate_unit_opt.unwrap_or_else(|| match span {
        Some(s) if s < 60.0 => RateUnit::Second,
        Some(s) if s < 3600.0 => RateUnit::Minute,
        _ => RateUnit::Hour,
    });
    let rate_of = |count: u64| -> Option<f64> {
        span.map(|s| count as f64 * unit.seconds() / s)
    };

    // ---- reduce ----
    let mut rows: Vec<Row> = aggs
        .into_iter()
        .map(|a| Row::new(a, parsed as f64, &pcts, method, rate_of(0).is_some()))
        .collect();
    for row in rows.iter_mut() {
        row.rate = rate_of(row.count);
    }
    let groups_found = rows.len();
    sort_rows(&mut rows, &sort);
    let mut shown: Vec<Row> = Vec::new();
    let mut rest: Vec<Row> = Vec::new();
    for (i, row) in rows.into_iter().enumerate() {
        if i < limit as usize {
            shown.push(row);
        } else {
            rest.push(row);
        }
    }
    let other_row = if other && !rest.is_empty() {
        let label = if group_fields.is_empty() {
            vec![OTHER.to_string()]
        } else {
            group_fields.iter().map(|_| OTHER.to_string()).collect()
        };
        let mut merged = Agg::new(label);
        for r in &rest {
            merged.count += r.count;
            merged.errors += r.errors;
            merged.values.extend_from_slice(&r.values);
        }
        let mut row = Row::new(merged, parsed as f64, &pcts, method, span.is_some());
        row.rate = rate_of(row.count);
        Some(row)
    } else {
        None
    };

    // Totals over every parsed row, truncation or not.
    let mut totals_agg = Agg::new(vec!["total".into()]);
    for r in shown.iter().chain(other_row.iter()).chain(rest.iter()) {
        totals_agg.count += r.count;
        totals_agg.errors += r.errors;
        totals_agg.values.extend_from_slice(&r.values);
    }
    // `other` already merged `rest`; drop the duplicate when it exists.
    if other_row.is_some() {
        totals_agg.count -= rest.iter().map(|r| r.count).sum::<u64>();
        totals_agg.errors -= rest.iter().map(|r| r.errors).sum::<u64>();
        let keep = totals_agg.values.len() - rest.iter().map(|r| r.values.len()).sum::<usize>();
        totals_agg.values.truncate(keep);
    }
    let mut totals = Row::new(totals_agg, parsed as f64, &pcts, method, span.is_some());
    totals.rate = rate_of(totals.count);

    let ctx = Ctx {
        fmt,
        group_fields: &group_fields,
        value_field,
        error_field,
        pcts: &pcts,
        method,
        unit,
        span,
        time_field: if time_disabled {
            None
        } else {
            chosen_time.as_deref()
        },
        total_lines,
        parsed,
        unparsed,
        value_missing,
        value_unparsed,
        groups_found,
        prefix: &prefix,
        rules_text: describe_rules(error_values),
    };

    Ok(match output.as_str() {
        "json" => render_json(&ctx, &shown, other_row.as_ref(), &totals),
        "csv" => render_csv(&ctx, &shown, other_row.as_ref()),
        "prometheus" => render_prometheus(&ctx, &shown, other_row.as_ref()),
        _ => render_table(&ctx, &shown, other_row.as_ref()),
    })
}

// ---------------------------------------------------------------------------
// Accumulators.
// ---------------------------------------------------------------------------

struct Agg {
    key: Vec<String>,
    count: u64,
    errors: u64,
    values: Vec<f64>,
}

impl Agg {
    fn new(key: Vec<String>) -> Self {
        Agg {
            key,
            count: 0,
            errors: 0,
            values: Vec::new(),
        }
    }
}

/// One finished group: counts plus the reduced value statistics.
struct Row {
    key: Vec<String>,
    count: u64,
    percent: f64,
    errors: u64,
    error_percent: f64,
    rate: Option<f64>,
    values: Vec<f64>,
    stats: Option<Stats>,
}

/// Reduced statistics of the numeric value field within one group.
struct Stats {
    n: usize,
    min: f64,
    avg: f64,
    max: f64,
    sum: f64,
    pcts: Vec<f64>,
}

impl Row {
    fn new(a: Agg, total: f64, pcts: &[f64], method: PctMethod, _has_span: bool) -> Row {
        let mut values = a.values;
        let stats = if values.is_empty() {
            None
        } else {
            values.sort_by(|x, y| x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal));
            let sum: f64 = values.iter().sum();
            Some(Stats {
                n: values.len(),
                min: values[0],
                avg: sum / values.len() as f64,
                max: values[values.len() - 1],
                sum,
                pcts: pcts.iter().map(|p| percentile(&values, *p, method)).collect(),
            })
        };
        Row {
            percent: if total > 0.0 {
                a.count as f64 * 100.0 / total
            } else {
                0.0
            },
            error_percent: if a.count > 0 {
                a.errors as f64 * 100.0 / a.count as f64
            } else {
                0.0
            },
            key: a.key,
            count: a.count,
            errors: a.errors,
            rate: None,
            values,
            stats,
        }
    }

    fn stat(&self, pick: &str) -> f64 {
        match (&self.stats, pick) {
            (Some(s), "sum") => s.sum,
            (Some(s), "avg") => s.avg,
            (Some(s), "max") => s.max,
            (Some(s), "p_top") => *s.pcts.last().unwrap_or(&f64::NEG_INFINITY),
            _ => f64::NEG_INFINITY,
        }
    }
}

/// Everything the renderers need that isn't per-row.
struct Ctx<'a> {
    fmt: Format,
    group_fields: &'a [String],
    value_field: &'a str,
    error_field: &'a str,
    pcts: &'a [f64],
    method: PctMethod,
    unit: RateUnit,
    span: Option<f64>,
    time_field: Option<&'a str>,
    total_lines: usize,
    parsed: usize,
    unparsed: usize,
    value_missing: usize,
    value_unparsed: usize,
    groups_found: usize,
    prefix: &'a str,
    rules_text: Vec<String>,
}

impl Ctx<'_> {
    fn group_headers(&self) -> Vec<String> {
        if self.group_fields.is_empty() {
            vec!["group".into()]
        } else {
            self.group_fields.to_vec()
        }
    }
    fn has_rate(&self) -> bool {
        self.span.is_some()
    }
    fn has_values(&self) -> bool {
        !self.value_field.is_empty()
    }
    fn has_errors(&self) -> bool {
        !self.error_field.is_empty()
    }
    fn pct_labels(&self) -> Vec<String> {
        self.pcts.iter().map(|p| format!("p{}", fmt_num(*p))).collect()
    }
}

fn sort_rows(rows: &mut [Row], sort: &str) {
    match sort {
        "group" => rows.sort_by(|a, b| a.key.cmp(&b.key)),
        "errors" => rows.sort_by(|a, b| b.errors.cmp(&a.errors).then_with(|| a.key.cmp(&b.key))),
        "sum" | "avg" | "max" | "p_top" => rows.sort_by(|a, b| {
            b.stat(sort)
                .partial_cmp(&a.stat(sort))
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.key.cmp(&b.key))
        }),
        _ => rows.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.key.cmp(&b.key))),
    }
}

// ---------------------------------------------------------------------------
// Option parsing.
// ---------------------------------------------------------------------------

fn norm(v: &str, default: &str) -> String {
    let t = v.trim();
    if t.is_empty() {
        default.to_ascii_lowercase()
    } else {
        t.to_ascii_lowercase()
    }
}

fn parse_percentiles(spec: &str) -> Result<Vec<f64>, String> {
    let spec = spec.trim();
    let spec = if spec.is_empty() { "50,95,99" } else { spec };
    let mut out = Vec::new();
    for part in spec.split(',') {
        let t = part.trim().trim_start_matches(['p', 'P']);
        if t.is_empty() {
            continue;
        }
        let v: f64 = t
            .parse()
            .map_err(|_| format!("invalid percentile {:?}: expected a number 0-100", part.trim()))?;
        if !(0.0..=100.0).contains(&v) || !v.is_finite() {
            return Err(format!("invalid percentile {v}: expected 0-100"));
        }
        out.push(v);
    }
    if out.is_empty() {
        return Err("no percentiles given: use something like '50,95,99'".into());
    }
    if out.len() > MAX_PERCENTILES {
        return Err(format!(
            "too many percentiles: {}, the maximum is {MAX_PERCENTILES}",
            out.len()
        ));
    }
    Ok(out)
}

fn parse_group_fields(spec: &str) -> Result<Vec<String>, String> {
    let fields: Vec<String> = spec
        .split(',')
        .map(|f| f.trim().to_string())
        .filter(|f| !f.is_empty())
        .collect();
    if fields.len() > MAX_GROUP_FIELDS {
        return Err(format!(
            "too many group_by fields: {}, the maximum is {MAX_GROUP_FIELDS}",
            fields.len()
        ));
    }
    Ok(fields)
}

fn parse_error_rules(spec: &str) -> Result<Vec<ErrorRule>, String> {
    let spec = spec.trim();
    let items: Vec<String> = if spec.is_empty() {
        DEFAULT_ERROR_VALUES.iter().map(|s| s.to_string()).collect()
    } else {
        spec.split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    };
    if items.is_empty() {
        return Err("error_values is empty: list the values that count as errors, e.g. '5*,error'".into());
    }
    let mut rules = Vec::new();
    for item in items {
        let rule = if let Some(rest) = item.strip_prefix(">=") {
            ErrorRule::Cmp(">=", num_rule(rest, &item)?)
        } else if let Some(rest) = item.strip_prefix("<=") {
            ErrorRule::Cmp("<=", num_rule(rest, &item)?)
        } else if let Some(rest) = item.strip_prefix('>') {
            ErrorRule::Cmp(">", num_rule(rest, &item)?)
        } else if let Some(rest) = item.strip_prefix('<') {
            ErrorRule::Cmp("<", num_rule(rest, &item)?)
        } else if let Some(rest) = item.strip_suffix('*') {
            ErrorRule::Prefix(rest.to_ascii_lowercase())
        } else {
            ErrorRule::Eq(item.clone())
        };
        rules.push(rule);
    }
    Ok(rules)
}

fn num_rule(rest: &str, item: &str) -> Result<f64, String> {
    rest.trim()
        .parse::<f64>()
        .map_err(|_| format!("invalid error_values entry {item:?}: expected a number after the comparison, e.g. '>=500'"))
}

fn describe_rules(spec: &str) -> Vec<String> {
    let spec = spec.trim();
    if spec.is_empty() {
        DEFAULT_ERROR_VALUES.iter().map(|s| s.to_string()).collect()
    } else {
        spec.split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Line parsing.
// ---------------------------------------------------------------------------

type Fields = Vec<(String, String)>;

fn lookup<'a>(fields: &'a Fields, name: &str) -> Option<&'a str> {
    fields
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_str())
}

fn detect_format(lines: &[&str]) -> Format {
    let sample: Vec<&str> = lines
        .iter()
        .copied()
        .filter(|l| !l.trim().is_empty())
        .take(5)
        .collect();
    if sample.iter().any(|l| l.trim_start().starts_with('{')) {
        return Format::Json;
    }
    let logfmt_like = sample
        .iter()
        .filter(|l| parse_logfmt_line(l).is_some())
        .count();
    if logfmt_like > 0 && logfmt_like * 2 >= sample.len() {
        return Format::Logfmt;
    }
    Format::Csv
}

fn parse_json_line(line: &str) -> Option<Fields> {
    let v: Value = serde_json::from_str(line.trim()).ok()?;
    let obj = v.as_object()?;
    let mut out = Vec::new();
    flatten_json(obj, "", &mut out);
    Some(out)
}

fn flatten_json(obj: &Map<String, Value>, prefix: &str, out: &mut Fields) {
    for (k, v) in obj {
        let key = if prefix.is_empty() {
            k.clone()
        } else {
            format!("{prefix}.{k}")
        };
        match v {
            Value::Object(inner) => flatten_json(inner, &key, out),
            Value::Null => {}
            Value::String(s) => out.push((key, s.clone())),
            Value::Bool(b) => out.push((key, b.to_string())),
            Value::Number(n) => out.push((key, n.to_string())),
            Value::Array(_) => out.push((key, v.to_string())),
        }
    }
}

/// logfmt: `key=value` pairs separated by whitespace; values may be
/// double-quoted (with `\"` escapes). Bare tokens without `=` are ignored, and
/// a line with no pair at all counts as unparsed.
fn parse_logfmt_line(line: &str) -> Option<Fields> {
    let chars: Vec<char> = line.chars().collect();
    let mut out: Fields = Vec::new();
    let mut i = 0usize;
    while i < chars.len() {
        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }
        if i >= chars.len() {
            break;
        }
        let start = i;
        while i < chars.len() && chars[i] != '=' && !chars[i].is_whitespace() {
            i += 1;
        }
        let key: String = chars[start..i].iter().collect();
        if i < chars.len() && chars[i] == '=' {
            i += 1;
            let mut value = String::new();
            if i < chars.len() && chars[i] == '"' {
                i += 1;
                while i < chars.len() {
                    if chars[i] == '\\' && i + 1 < chars.len() {
                        value.push(chars[i + 1]);
                        i += 2;
                        continue;
                    }
                    if chars[i] == '"' {
                        i += 1;
                        break;
                    }
                    value.push(chars[i]);
                    i += 1;
                }
            } else {
                while i < chars.len() && !chars[i].is_whitespace() {
                    value.push(chars[i]);
                    i += 1;
                }
            }
            if !key.is_empty() {
                out.push((key, value));
            }
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

fn detect_delimiter(header: &str) -> char {
    let commas = header.matches(',').count();
    let tabs = header.matches('\t').count();
    let semis = header.matches(';').count();
    if tabs >= commas && tabs >= semis && tabs > 0 {
        '\t'
    } else if semis > commas && semis > 0 {
        ';'
    } else {
        ','
    }
}

/// RFC 4180-ish split: `"` quotes a field, `""` is a literal quote.
fn split_delimited(line: &str, delim: char) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if in_quotes {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    cur.push('"');
                    chars.next();
                } else {
                    in_quotes = false;
                }
            } else {
                cur.push(c);
            }
        } else if c == '"' && cur.trim().is_empty() {
            cur.clear();
            in_quotes = true;
        } else if c == delim {
            out.push(std::mem::take(&mut cur));
        } else {
            cur.push(c);
        }
    }
    out.push(cur);
    out
}

fn zip_csv(header: &[String], cells: &[String]) -> Fields {
    let mut out = Vec::new();
    for (i, cell) in cells.iter().enumerate() {
        let key = header
            .get(i)
            .cloned()
            .unwrap_or_else(|| format!("column{}", i + 1));
        if key.is_empty() {
            continue;
        }
        out.push((key, cell.trim().to_string()));
    }
    out
}

// ---------------------------------------------------------------------------
// Value + time parsing.
// ---------------------------------------------------------------------------

/// Duration suffixes recognised on a value, longest first, with their factor to
/// milliseconds. A bare number is used as-is (no unit assumed).
const DURATION_UNITS: &[(&str, f64)] = &[
    ("µs", 0.001),
    ("us", 0.001),
    ("ms", 1.0),
    ("ns", 0.000_001),
    ("s", 1000.0),
    ("m", 60_000.0),
    ("h", 3_600_000.0),
];

/// Parse a numeric field value. Plain numbers pass through unchanged; a value
/// with a duration suffix (`250ms`, `1.5s`) is normalised to milliseconds.
pub fn parse_number(raw: &str) -> Option<f64> {
    let s = raw.trim().trim_matches('"').trim();
    if s.is_empty() {
        return None;
    }
    if let Ok(v) = s.parse::<f64>() {
        return v.is_finite().then_some(v);
    }
    let lower = s.to_ascii_lowercase();
    for (suffix, factor) in DURATION_UNITS {
        if let Some(head) = lower.strip_suffix(suffix) {
            if let Ok(v) = head.trim().parse::<f64>() {
                if v.is_finite() {
                    return Some(v * factor);
                }
            }
        }
    }
    None
}

/// Parse a timestamp field into epoch seconds. Accepts epoch numbers
/// (seconds/millis/micros/nanos, auto-scaled), ISO-8601 / RFC-3339
/// (`2024-05-06T07:08:09.123Z`, `2024-05-06 07:08:09`, `2024-05-06`) with an
/// optional `Z`/`±HH:MM` offset, and the Apache/nginx `10/Oct/2000:13:55:36
/// -0700` form.
pub fn parse_time(raw: &str) -> Option<f64> {
    let s = raw.trim().trim_matches('"').trim().trim_matches(['[', ']']);
    if s.is_empty() {
        return None;
    }
    // Epoch numbers.
    if s.chars()
        .all(|c| c.is_ascii_digit() || c == '.' || c == '-' || c == '+')
        && s.chars().any(|c| c.is_ascii_digit())
    {
        if let Ok(v) = s.parse::<f64>() {
            let a = v.abs();
            let scaled = if a >= 1e17 {
                v / 1e9
            } else if a >= 1e14 {
                v / 1e6
            } else if a >= 1e11 {
                v / 1e3
            } else {
                v
            };
            return Some(scaled);
        }
    }
    if let Some(t) = parse_clf_time(s) {
        return Some(t);
    }
    parse_iso_time(s)
}

fn parse_iso_time(s: &str) -> Option<f64> {
    let bytes: Vec<char> = s.chars().collect();
    if bytes.len() < 10 {
        return None;
    }
    let year: i64 = s.get(0..4)?.parse().ok()?;
    if bytes[4] != '-' || bytes[7] != '-' {
        return None;
    }
    let month: i64 = s.get(5..7)?.parse().ok()?;
    let day: i64 = s.get(8..10)?.parse().ok()?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    let mut secs = days_from_civil(year, month, day) as f64 * 86_400.0;
    let rest = &s[10..];
    if rest.is_empty() {
        return Some(secs);
    }
    let rest = rest.strip_prefix('T').or_else(|| rest.strip_prefix('t')).or_else(|| rest.strip_prefix(' '))?;
    if rest.len() < 5 {
        return None;
    }
    let hh: i64 = rest.get(0..2)?.parse().ok()?;
    let mm: i64 = rest.get(3..5)?.parse().ok()?;
    if rest.as_bytes()[2] != b':' || hh > 23 || mm > 59 {
        return None;
    }
    secs += (hh * 3600 + mm * 60) as f64;
    let mut tail = &rest[5..];
    if tail.starts_with(':') && tail.len() >= 3 {
        let ss: f64 = tail.get(1..3)?.parse().ok()?;
        secs += ss;
        tail = &tail[3..];
        if let Some(frac) = tail.strip_prefix('.') {
            let digits: String = frac.chars().take_while(|c| c.is_ascii_digit()).collect();
            if !digits.is_empty() {
                secs += format!("0.{digits}").parse::<f64>().ok()?;
                tail = &tail[1 + digits.len()..];
            }
        }
    }
    secs -= offset_seconds(tail)?;
    Some(secs)
}

/// `+HH:MM` / `-HHMM` / `Z` / empty (treated as UTC) → offset in seconds.
fn offset_seconds(tail: &str) -> Option<f64> {
    let t = tail.trim();
    if t.is_empty() || t.eq_ignore_ascii_case("z") {
        return Some(0.0);
    }
    let sign = match t.chars().next()? {
        '+' => 1.0,
        '-' => -1.0,
        _ => return None,
    };
    let digits: String = t[1..].chars().filter(|c| c.is_ascii_digit()).collect();
    let (h, m) = match digits.len() {
        2 => (digits.parse::<f64>().ok()?, 0.0),
        4 => (
            digits[0..2].parse::<f64>().ok()?,
            digits[2..4].parse::<f64>().ok()?,
        ),
        _ => return None,
    };
    Some(sign * (h * 3600.0 + m * 60.0))
}

const MONTHS: &[&str] = &[
    "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
];

/// `10/Oct/2000:13:55:36 -0700` (Apache/nginx common log format).
fn parse_clf_time(s: &str) -> Option<f64> {
    let (date_part, rest) = s.split_once(':')?;
    let mut parts = date_part.split('/');
    let day: i64 = parts.next()?.trim().parse().ok()?;
    let mon_name = parts.next()?.to_ascii_lowercase();
    let month = MONTHS.iter().position(|m| *m == mon_name)? as i64 + 1;
    let year: i64 = parts.next()?.trim().parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    let mut clock = rest.split_whitespace();
    let hms = clock.next()?;
    let mut hp = hms.split(':');
    let hh: i64 = hp.next()?.parse().ok()?;
    let mm: i64 = hp.next()?.parse().ok()?;
    let ss: f64 = hp.next().unwrap_or("0").parse().ok()?;
    let mut secs =
        days_from_civil(year, month, day) as f64 * 86_400.0 + (hh * 3600 + mm * 60) as f64 + ss;
    if let Some(off) = clock.next() {
        secs -= offset_seconds(off)?;
    }
    Some(secs)
}

/// Days since 1970-01-01 for a proleptic Gregorian date (Hinnant's algorithm).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

// ---------------------------------------------------------------------------
// Statistics.
// ---------------------------------------------------------------------------

/// Exact percentile of an already-sorted slice.
fn percentile(sorted: &[f64], p: f64, method: PctMethod) -> f64 {
    let n = sorted.len();
    if n == 0 {
        return f64::NAN;
    }
    match method {
        PctMethod::Linear => {
            let pos = p / 100.0 * (n - 1) as f64;
            let lo = pos.floor() as usize;
            let hi = pos.ceil() as usize;
            if lo == hi {
                sorted[lo]
            } else {
                sorted[lo] + (sorted[hi] - sorted[lo]) * (pos - lo as f64)
            }
        }
        PctMethod::Nearest => {
            let rank = (p / 100.0 * n as f64).ceil() as usize;
            sorted[rank.clamp(1, n) - 1]
        }
    }
}

// ---------------------------------------------------------------------------
// Formatting helpers.
// ---------------------------------------------------------------------------

/// Compact number rendering: integers print bare, everything else keeps up to
/// three significant decimals with trailing zeros trimmed.
fn fmt_num(v: f64) -> String {
    if !v.is_finite() {
        return "-".into();
    }
    if v == v.trunc() && v.abs() < 1e15 {
        return format!("{}", v as i64);
    }
    let magnitude = v.abs();
    let decimals = if magnitude >= 100.0 {
        2
    } else if magnitude >= 1.0 {
        3
    } else {
        6
    };
    let s = format!("{v:.decimals$}");
    let s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    if s.is_empty() || s == "-" {
        "0".into()
    } else {
        s
    }
}

fn fmt_pct(v: f64) -> String {
    fmt_num((v * 100.0).round() / 100.0)
}

fn fmt_span(span: Option<f64>) -> String {
    match span {
        None => "n/a".into(),
        Some(s) => format!("{}s", fmt_num((s * 1000.0).round() / 1000.0)),
    }
}

// ---------------------------------------------------------------------------
// Renderers.
// ---------------------------------------------------------------------------

fn columns(ctx: &Ctx) -> Vec<String> {
    let mut cols = ctx.group_headers();
    cols.push("count".into());
    cols.push("percent".into());
    if ctx.has_rate() {
        cols.push(ctx.unit.column().into());
    }
    if ctx.has_errors() {
        cols.push("errors".into());
        cols.push("error%".into());
    }
    if ctx.has_values() {
        cols.push("min".into());
        cols.push("avg".into());
        cols.extend(ctx.pct_labels());
        cols.push("max".into());
        cols.push("sum".into());
    }
    cols
}

fn row_cells(ctx: &Ctx, row: &Row) -> Vec<String> {
    let mut cells = row.key.clone();
    cells.push(row.count.to_string());
    cells.push(fmt_pct(row.percent));
    if ctx.has_rate() {
        cells.push(row.rate.map(fmt_num).unwrap_or_else(|| "-".into()));
    }
    if ctx.has_errors() {
        cells.push(row.errors.to_string());
        cells.push(fmt_pct(row.error_percent));
    }
    if ctx.has_values() {
        match &row.stats {
            None => {
                for _ in 0..(4 + ctx.pcts.len()) {
                    cells.push("-".into());
                }
            }
            Some(s) => {
                cells.push(fmt_num(s.min));
                cells.push(fmt_num(s.avg));
                for p in &s.pcts {
                    cells.push(fmt_num(*p));
                }
                cells.push(fmt_num(s.max));
                cells.push(fmt_num(s.sum));
            }
        }
    }
    cells
}

fn summary_line(ctx: &Ctx) -> String {
    let mut s = format!(
        "lines={} parsed={} unparsed={} format={} groups={} span={}",
        ctx.total_lines,
        ctx.parsed,
        ctx.unparsed,
        ctx.fmt.name(),
        ctx.groups_found,
        fmt_span(ctx.span)
    );
    if let Some(tf) = ctx.time_field {
        if ctx.span.is_some() {
            s.push_str(&format!(" time_field={tf}"));
        }
    }
    if ctx.has_values() {
        s.push_str(&format!(
            " value={} n={} skipped={}",
            ctx.value_field,
            ctx.parsed - ctx.value_missing - ctx.value_unparsed,
            ctx.value_missing + ctx.value_unparsed
        ));
    }
    s
}

fn render_table(ctx: &Ctx, rows: &[Row], other: Option<&Row>) -> String {
    let header = columns(ctx);
    let mut body: Vec<Vec<String>> = rows.iter().map(|r| row_cells(ctx, r)).collect();
    if let Some(o) = other {
        body.push(row_cells(ctx, o));
    }
    let ngroups = ctx.group_headers().len();
    let mut widths: Vec<usize> = header.iter().map(|h| h.chars().count()).collect();
    for r in &body {
        for (i, c) in r.iter().enumerate() {
            widths[i] = widths[i].max(c.chars().count());
        }
    }
    let pad = |s: &str, w: usize, left: bool| {
        let fill = w.saturating_sub(s.chars().count());
        if left {
            format!("{s}{}", " ".repeat(fill))
        } else {
            format!("{}{s}", " ".repeat(fill))
        }
    };
    let mut out = String::new();
    out.push_str(&summary_line(ctx));
    out.push_str("\n\n");
    let render = |cells: &[String]| -> String {
        let mut line = String::from("|");
        for (i, c) in cells.iter().enumerate() {
            line.push(' ');
            line.push_str(&pad(c, widths[i], i < ngroups));
            line.push_str(" |");
        }
        line
    };
    out.push_str(&render(&header));
    out.push('\n');
    let sep: Vec<String> = widths
        .iter()
        .enumerate()
        .map(|(i, w)| {
            if i < ngroups {
                "-".repeat(*w)
            } else {
                format!("{}:", "-".repeat(w.saturating_sub(1)))
            }
        })
        .collect();
    out.push_str(&render(&sep));
    for r in &body {
        out.push('\n');
        out.push_str(&render(r));
    }
    out
}

fn render_csv(ctx: &Ctx, rows: &[Row], other: Option<&Row>) -> String {
    let esc = |s: &str| -> String {
        if s.contains([',', '"', '\n']) {
            format!("\"{}\"", s.replace('"', "\"\""))
        } else {
            s.to_string()
        }
    };
    let mut out = String::new();
    out.push_str(
        &columns(ctx)
            .iter()
            .map(|c| esc(c))
            .collect::<Vec<_>>()
            .join(","),
    );
    for r in rows.iter().chain(other) {
        out.push('\n');
        out.push_str(
            &row_cells(ctx, r)
                .iter()
                .map(|c| esc(c))
                .collect::<Vec<_>>()
                .join(","),
        );
    }
    out
}

fn row_json(ctx: &Ctx, row: &Row) -> Value {
    let mut group = Map::new();
    if ctx.group_fields.is_empty() {
        group.insert("group".into(), json!(row.key[0]));
    } else {
        for (f, v) in ctx.group_fields.iter().zip(row.key.iter()) {
            group.insert(f.clone(), json!(v));
        }
    }
    let mut obj = Map::new();
    obj.insert("group".into(), Value::Object(group));
    obj.insert("count".into(), json!(row.count));
    obj.insert("percent".into(), json!(round2(row.percent)));
    if ctx.has_rate() {
        obj.insert("rate".into(), json!(row.rate.map(round3)));
    }
    if ctx.has_errors() {
        obj.insert("errors".into(), json!(row.errors));
        obj.insert("error_percent".into(), json!(round2(row.error_percent)));
    }
    if ctx.has_values() {
        obj.insert(
            "value".into(),
            match &row.stats {
                None => Value::Null,
                Some(s) => {
                    let mut pcts = Map::new();
                    for (label, v) in ctx.pct_labels().iter().zip(s.pcts.iter()) {
                        pcts.insert(label.clone(), json!(round3(*v)));
                    }
                    json!({
                        "n": s.n,
                        "min": round3(s.min),
                        "avg": round3(s.avg),
                        "max": round3(s.max),
                        "sum": round3(s.sum),
                        "percentiles": Value::Object(pcts),
                    })
                }
            },
        );
    }
    Value::Object(obj)
}

fn render_json(ctx: &Ctx, rows: &[Row], other: Option<&Row>, totals: &Row) -> String {
    let mut root = Map::new();
    root.insert("format".into(), json!(ctx.fmt.name()));
    root.insert("lines".into(), json!(ctx.total_lines));
    root.insert("parsed".into(), json!(ctx.parsed));
    root.insert("unparsed".into(), json!(ctx.unparsed));
    root.insert(
        "group_by".into(),
        json!(if ctx.group_fields.is_empty() {
            Vec::<String>::new()
        } else {
            ctx.group_fields.to_vec()
        }),
    );
    root.insert("groups_found".into(), json!(ctx.groups_found));
    root.insert("groups_shown".into(), json!(rows.len()));
    root.insert("time_field".into(), json!(ctx.time_field));
    root.insert("span_seconds".into(), json!(ctx.span.map(round3)));
    root.insert("rate_unit".into(), json!(ctx.unit.name()));
    if ctx.has_values() {
        root.insert("value_field".into(), json!(ctx.value_field));
        root.insert(
            "value_parsed".into(),
            json!(ctx.parsed - ctx.value_missing - ctx.value_unparsed),
        );
        root.insert("value_missing".into(), json!(ctx.value_missing));
        root.insert("value_non_numeric".into(), json!(ctx.value_unparsed));
        root.insert("percentiles".into(), json!(ctx.pcts));
        root.insert(
            "percentile_method".into(),
            json!(match ctx.method {
                PctMethod::Linear => "linear",
                PctMethod::Nearest => "nearest",
            }),
        );
    }
    if ctx.has_errors() {
        root.insert("error_field".into(), json!(ctx.error_field));
        root.insert("error_values".into(), json!(ctx.rules_text));
    }
    root.insert("totals".into(), row_json(ctx, totals));
    root.insert(
        "groups".into(),
        Value::Array(rows.iter().map(|r| row_json(ctx, r)).collect()),
    );
    if let Some(o) = other {
        root.insert("other".into(), row_json(ctx, o));
    }
    serde_json::to_string_pretty(&Value::Object(root)).unwrap_or_else(|e| e.to_string())
}

fn render_prometheus(ctx: &Ctx, rows: &[Row], other: Option<&Row>) -> String {
    let all: Vec<&Row> = rows.iter().chain(other).collect();
    let labels = |row: &Row| -> String {
        if ctx.group_fields.is_empty() {
            String::new()
        } else {
            let inner: Vec<String> = ctx
                .group_fields
                .iter()
                .zip(row.key.iter())
                .map(|(f, v)| {
                    format!(
                        "{}=\"{}\"",
                        sanitize_metric(f, "label"),
                        v.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n")
                    )
                })
                .collect();
            format!("{{{}}}", inner.join(","))
        }
    };
    let with_extra = |base: &str, extra: &str| -> String {
        if base.is_empty() {
            format!("{{{extra}}}")
        } else {
            format!("{},{extra}}}", base.trim_end_matches('}'))
        }
    };
    let p = ctx.prefix;
    let mut out = String::new();
    out.push_str(&format!("# HELP {p}_lines_total Log lines in each group.\n"));
    out.push_str(&format!("# TYPE {p}_lines_total counter\n"));
    for r in &all {
        out.push_str(&format!("{p}_lines_total{} {}\n", labels(r), r.count));
    }
    if ctx.has_rate() {
        let suffix = ctx.unit.metric_suffix();
        out.push_str(&format!(
            "# HELP {p}_lines_{suffix} Log lines per {} over the {} second span of the input.\n",
            ctx.unit.name(),
            fmt_num(ctx.span.unwrap_or(0.0))
        ));
        out.push_str(&format!("# TYPE {p}_lines_{suffix} gauge\n"));
        for r in &all {
            out.push_str(&format!(
                "{p}_lines_{suffix}{} {}\n",
                labels(r),
                fmt_num(r.rate.unwrap_or(0.0))
            ));
        }
    }
    if ctx.has_errors() {
        out.push_str(&format!(
            "# HELP {p}_errors_total Lines whose {} field matched an error value.\n",
            ctx.error_field
        ));
        out.push_str(&format!("# TYPE {p}_errors_total counter\n"));
        for r in &all {
            out.push_str(&format!("{p}_errors_total{} {}\n", labels(r), r.errors));
        }
    }
    if ctx.has_values() {
        let name = format!("{p}_{}", sanitize_metric(ctx.value_field, "value"));
        out.push_str(&format!(
            "# HELP {name} Distribution of the {} field.\n",
            ctx.value_field
        ));
        out.push_str(&format!("# TYPE {name} summary\n"));
        for r in &all {
            let base = labels(r);
            if let Some(s) = &r.stats {
                for (pct, v) in ctx.pcts.iter().zip(s.pcts.iter()) {
                    out.push_str(&format!(
                        "{name}{} {}\n",
                        with_extra(&base, &format!("quantile=\"{}\"", fmt_num(pct / 100.0))),
                        fmt_num(*v)
                    ));
                }
                out.push_str(&format!("{name}_sum{base} {}\n", fmt_num(s.sum)));
                out.push_str(&format!("{name}_count{base} {}\n", s.n));
            }
        }
    }
    out.trim_end().to_string()
}

/// Prometheus metric/label names allow `[a-zA-Z_][a-zA-Z0-9_]*`.
fn sanitize_metric(raw: &str, fallback: &str) -> String {
    let mut s: String = raw
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    while s.starts_with('_') {
        s.remove(0);
    }
    while s.ends_with('_') {
        s.pop();
    }
    if s.is_empty() || s.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        return fallback.to_string();
    }
    s
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

fn round3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}

// ---------------------------------------------------------------------------
// Tests.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const JSON_LOG: &str = concat!(
        r#"{"ts":"2024-05-06T07:00:00Z","route":"/api/users","status":200,"duration_ms":10}"#,
        "\n",
        r#"{"ts":"2024-05-06T07:00:20Z","route":"/api/users","status":500,"duration_ms":30}"#,
        "\n",
        r#"{"ts":"2024-05-06T07:00:40Z","route":"/api/users","status":200,"duration_ms":20}"#,
        "\n",
        r#"{"ts":"2024-05-06T07:01:00Z","route":"/health","status":200,"duration_ms":2}"#,
    );

    fn run(data: &str, group: &str, value: &str, output: &str) -> String {
        aggregate(
            data, "auto", group, value, "50,95,99", "linear", "", "auto", "", "", 20, true,
            "count", output, "log",
        )
        .expect("aggregate should succeed")
    }

    #[test]
    fn json_grouped_counts_rates_and_percentiles() {
        let out = run(JSON_LOG, "route", "duration_ms", "table");
        // Span is 60 s → auto rate unit is per minute.
        assert!(out.contains("lines=4 parsed=4 unparsed=0 format=json groups=2 span=60s"));
        assert!(out.contains("time_field=ts"), "{out}");
        assert!(out.contains("rate/min"), "{out}");
        let users = out
            .lines()
            .find(|l| l.contains("/api/users"))
            .expect("group row");
        // count=3, percent=75, rate=3/min, min=10 avg=20 p50=20 max=30 sum=60
        assert!(users.contains("|     3 |"), "{users}");
        assert!(users.contains("|      75 |"), "{users}");
        assert!(users.contains("|  20 |"), "{users}");
        assert!(users.contains("|  60 |"), "{users}");
    }

    #[test]
    fn json_output_carries_totals_and_stats() {
        let raw = aggregate(
            JSON_LOG, "json", "route", "duration_ms", "50,90", "linear", "ts", "second", "status",
            "5*", 20, true, "count", "json", "log",
        )
        .unwrap();
        let v: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(v["parsed"], 4);
        assert_eq!(v["span_seconds"], 60.0);
        assert_eq!(v["rate_unit"], "second");
        assert_eq!(v["groups_found"], 2);
        assert_eq!(v["totals"]["count"], 4);
        assert_eq!(v["totals"]["errors"], 1);
        let g = &v["groups"][0];
        assert_eq!(g["group"]["route"], "/api/users");
        assert_eq!(g["count"], 3);
        assert_eq!(g["errors"], 1);
        assert_eq!(g["error_percent"], 33.33);
        assert_eq!(g["value"]["p50"], Value::Null); // percentiles live under "percentiles"
        assert_eq!(g["value"]["percentiles"]["p50"], 20.0);
        assert_eq!(g["value"]["percentiles"]["p90"], 28.0);
        assert_eq!(g["value"]["avg"], 20.0);
        assert_eq!(g["rate"], 0.05);
    }

    #[test]
    fn logfmt_with_duration_units_and_error_comparison() {
        let logs = concat!(
            "time=2024-05-06T07:00:00Z svc=api status=200 dur=250ms\n",
            "time=2024-05-06T07:00:10Z svc=api status=503 dur=1.5s\n",
            "time=2024-05-06T07:00:20Z svc=web status=200 dur=100ms\n",
        );
        let raw = aggregate(
            logs, "auto", "svc", "dur", "50", "linear", "", "second", "status", ">=500", 20, true,
            "count", "json", "log",
        )
        .unwrap();
        let v: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(v["format"], "logfmt");
        let api = &v["groups"][0];
        assert_eq!(api["group"]["svc"], "api");
        assert_eq!(api["errors"], 1);
        // 1.5s normalises to 1500 ms, so avg is (250+1500)/2.
        assert_eq!(api["value"]["avg"], 875.0);
        assert_eq!(api["value"]["max"], 1500.0);
    }

    #[test]
    fn csv_input_missing_group_values_and_nearest_rank() {
        let csv = "route,ms\n/a,1\n/a,2\n/a,3\n/a,4\n,9\n";
        let raw = aggregate(
            csv, "csv", "route", "ms", "50", "nearest", "none", "auto", "", "", 20, true, "count",
            "json", "log",
        )
        .unwrap();
        let v: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(v["parsed"], 5);
        assert_eq!(v["groups"][0]["group"]["route"], "/a");
        // nearest-rank p50 of [1,2,3,4] is the 2nd value; linear would give 2.5.
        assert_eq!(v["groups"][0]["value"]["percentiles"]["p50"], 2.0);
        assert_eq!(v["groups"][1]["group"]["route"], MISSING);
        assert_eq!(v["span_seconds"], Value::Null);
    }

    #[test]
    fn linear_and_nearest_percentiles_differ_as_documented() {
        let sorted = [1.0, 2.0, 3.0, 4.0];
        assert_eq!(percentile(&sorted, 50.0, PctMethod::Linear), 2.5);
        assert_eq!(percentile(&sorted, 50.0, PctMethod::Nearest), 2.0);
        assert_eq!(percentile(&sorted, 100.0, PctMethod::Nearest), 4.0);
        assert_eq!(percentile(&sorted, 0.0, PctMethod::Linear), 1.0);
    }

    #[test]
    fn limit_rolls_the_remainder_into_an_other_row() {
        let mut logs = String::new();
        for i in 0..5 {
            for _ in 0..(5 - i) {
                logs.push_str(&format!("route=/r{i} ms={}\n", i + 1));
            }
        }
        let raw = aggregate(
            &logs, "logfmt", "route", "ms", "50", "linear", "none", "auto", "", "", 2, true,
            "count", "json", "log",
        )
        .unwrap();
        let v: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(v["groups_found"], 5);
        assert_eq!(v["groups_shown"], 2);
        assert_eq!(v["other"]["count"], 6); // 3 + 2 + 1
        assert_eq!(v["other"]["group"]["route"], OTHER);
        assert_eq!(v["totals"]["count"], 15);
        // Turning the rollup off drops the row but keeps the found count.
        let raw = aggregate(
            &logs, "logfmt", "route", "ms", "50", "linear", "none", "auto", "", "", 2, false,
            "count", "json", "log",
        )
        .unwrap();
        let v: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(v["other"], Value::Null);
        assert_eq!(v["totals"]["count"], 15);
    }

    #[test]
    fn sorting_switches_the_ranking() {
        let logs = concat!(
            "route=/slow ms=900\n",
            "route=/fast ms=1\n",
            "route=/fast ms=2\n",
            "route=/fast ms=3\n",
        );
        let by_count = aggregate(
            logs, "logfmt", "route", "ms", "95", "linear", "none", "auto", "", "", 20, true,
            "count", "json", "log",
        )
        .unwrap();
        let v: Value = serde_json::from_str(&by_count).unwrap();
        assert_eq!(v["groups"][0]["group"]["route"], "/fast");
        let by_p = aggregate(
            logs, "logfmt", "route", "ms", "95", "linear", "none", "auto", "", "", 20, true,
            "p_top", "json", "log",
        )
        .unwrap();
        let v: Value = serde_json::from_str(&by_p).unwrap();
        assert_eq!(v["groups"][0]["group"]["route"], "/slow");
        let by_group = aggregate(
            logs, "logfmt", "route", "ms", "95", "linear", "none", "auto", "", "", 20, true,
            "group", "json", "log",
        )
        .unwrap();
        let v: Value = serde_json::from_str(&by_group).unwrap();
        assert_eq!(v["groups"][0]["group"]["route"], "/fast");
    }

    #[test]
    fn prometheus_exposition_uses_summary_quantiles() {
        let out = aggregate(
            JSON_LOG, "json", "route", "duration_ms", "95", "linear", "ts", "second", "status",
            "5*", 20, true, "count", "prometheus", "svc",
        )
        .unwrap();
        assert!(out.contains("# TYPE svc_lines_total counter"), "{out}");
        assert!(
            out.contains("svc_lines_total{route=\"/api/users\"} 3"),
            "{out}"
        );
        assert!(
            out.contains("svc_errors_total{route=\"/api/users\"} 1"),
            "{out}"
        );
        assert!(out.contains("# TYPE svc_duration_ms summary"), "{out}");
        assert!(
            out.contains("svc_duration_ms{route=\"/api/users\",quantile=\"0.95\"} 29"),
            "{out}"
        );
        assert!(
            out.contains("svc_duration_ms_count{route=\"/api/users\"} 3"),
            "{out}"
        );
    }

    #[test]
    fn no_group_by_folds_everything_into_one_row() {
        let out = run(JSON_LOG, "", "duration_ms", "csv");
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[0], "group,count,percent,rate/min,min,avg,p50,p95,p99,max,sum");
        assert!(lines[1].starts_with("(all),4,100,4,"), "{}", lines[1]);
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn nested_json_fields_are_reachable_by_dotted_path() {
        let logs = concat!(
            r#"{"http":{"status":200,"route":"/a"},"took":5}"#,
            "\n",
            r#"{"http":{"status":500,"route":"/a"},"took":7}"#,
            "\n",
        );
        let raw = aggregate(
            logs,
            "json",
            "http.route",
            "took",
            "50",
            "linear",
            "none",
            "auto",
            "http.status",
            ">=500",
            20,
            true,
            "count",
            "json",
            "log",
        )
        .unwrap();
        let v: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(v["groups"][0]["group"]["http.route"], "/a");
        assert_eq!(v["groups"][0]["errors"], 1);
        assert_eq!(v["groups"][0]["value"]["avg"], 6.0);
    }

    #[test]
    fn unparseable_lines_are_counted_not_fatal() {
        let logs = concat!(
            r#"{"route":"/a","ms":1}"#,
            "\n",
            "this line is not json\n",
            r#"{"route":"/a","ms":3}"#,
            "\n",
        );
        let raw = run(logs, "route", "ms", "json");
        let v: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(v["lines"], 3);
        assert_eq!(v["parsed"], 2);
        assert_eq!(v["unparsed"], 1);
    }

    #[test]
    fn non_numeric_values_are_reported() {
        let logs = "route=/a ms=1\nroute=/a ms=oops\nroute=/a\n";
        let raw = run(logs, "route", "ms", "json");
        let v: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(v["value_parsed"], 1);
        assert_eq!(v["value_non_numeric"], 1);
        assert_eq!(v["value_missing"], 1);
    }

    #[test]
    fn epoch_and_clf_timestamps_parse() {
        assert_eq!(parse_time("1714982400"), Some(1_714_982_400.0));
        assert_eq!(parse_time("1714982400123"), Some(1_714_982_400.123));
        assert_eq!(parse_time("2024-05-06T07:00:00Z"), Some(1_714_978_800.0));
        assert_eq!(parse_time("2024-05-06 07:00:00"), Some(1_714_978_800.0));
        assert_eq!(
            parse_time("2024-05-06T09:00:00+02:00"),
            Some(1_714_978_800.0)
        );
        assert_eq!(parse_time("[10/Oct/2000:13:55:36 -0700]"), Some(971_211_336.0));
        assert_eq!(parse_time("not a time"), None);
    }

    #[test]
    fn duration_units_normalise_to_milliseconds() {
        assert_eq!(parse_number("250"), Some(250.0));
        assert_eq!(parse_number("250ms"), Some(250.0));
        assert_eq!(parse_number("1.5s"), Some(1500.0));
        assert_eq!(parse_number("900us"), Some(0.9));
        assert_eq!(parse_number("2h"), Some(7_200_000.0));
        assert_eq!(parse_number("-1e2"), Some(-100.0));
        assert_eq!(parse_number("abc"), None);
    }

    // ---- error paths -----------------------------------------------------

    #[test]
    fn empty_input_is_an_error() {
        let err = run_err("   ", "route", "table");
        assert!(err.contains("no input"), "{err}");
    }

    #[test]
    fn unparseable_input_is_an_error() {
        let err = aggregate(
            "just some prose\nmore prose\n",
            "json",
            "",
            "",
            "50",
            "linear",
            "none",
            "auto",
            "",
            "",
            20,
            true,
            "count",
            "table",
            "log",
        )
        .unwrap_err();
        assert!(err.contains("no json records found"), "{err}");
    }

    #[test]
    fn invalid_knobs_are_rejected_with_guidance() {
        assert!(run_err_opt("output", "xml").contains("expected 'table', 'json', 'csv'"));
        assert!(run_err_opt("sort", "median").contains("expected 'count'"));
        assert!(run_err_opt("percentile_method", "tdigest").contains("'linear' or 'nearest'"));
        assert!(run_err_opt("rate_unit", "day").contains("'auto', 'second'"));
        assert!(run_err_opt("format", "xml").contains("expected 'auto', 'json'"));
        assert!(run_err_opt("percentiles", "101").contains("expected 0-100"));
        assert!(run_err_opt("percentiles", "1,2,3,4,5,6,7,8,9,10,11").contains("too many percentiles"));
        assert!(run_err_opt("group_by", "a,b,c,d,e,f").contains("too many group_by fields"));
        assert!(run_err_opt("error_values", ">=abc").contains("expected a number after"));
    }

    #[test]
    fn percentile_and_group_caps_accept_the_boundary() {
        // Exactly MAX_PERCENTILES and MAX_GROUP_FIELDS are fine.
        assert!(aggregate(
            "a=1 b=2 c=3 d=4 e=5 ms=7",
            "logfmt",
            "a,b,c,d,e",
            "ms",
            "1,2,3,4,5,6,7,8,9,10",
            "linear",
            "none",
            "auto",
            "",
            "",
            20,
            true,
            "count",
            "table",
            "log",
        )
        .is_ok());
    }

    #[test]
    fn line_cap_is_enforced() {
        let over = "a=1\n".repeat(MAX_LINES + 1);
        let err = aggregate(
            &over, "logfmt", "a", "", "50", "linear", "none", "auto", "", "", 20, true, "count",
            "table", "log",
        )
        .unwrap_err();
        assert!(err.contains("too many lines"), "{err}");
    }

    #[test]
    fn limit_range_is_enforced() {
        let err = aggregate(
            "a=1", "logfmt", "a", "", "50", "linear", "none", "auto", "", "", 0, true, "count",
            "table", "log",
        )
        .unwrap_err();
        assert!(err.contains("invalid limit 0"), "{err}");
    }

    fn run_err(data: &str, group: &str, output: &str) -> String {
        aggregate(
            data, "auto", group, "", "50", "linear", "none", "auto", "", "", 20, true, "count",
            output, "log",
        )
        .unwrap_err()
    }

    fn run_err_opt(which: &str, value: &str) -> String {
        let data = "route=/a ms=1";
        let mut args = [
            "auto",  // format
            "route", // group_by
            "ms",    // value_field
            "50",    // percentiles
            "linear",
            "none", // time_field
            "auto", // rate_unit
            "",     // error_field
            "",     // error_values
            "count",
            "table",
        ];
        let slot = match which {
            "format" => 0,
            "group_by" => 1,
            "value_field" => 2,
            "percentiles" => 3,
            "percentile_method" => 4,
            "time_field" => 5,
            "rate_unit" => 6,
            "error_field" => 7,
            "error_values" => 8,
            "sort" => 9,
            "output" => 10,
            other => panic!("unknown option {other}"),
        };
        args[slot] = value;
        aggregate(
            data, args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8],
            20, true, args[9], args[10], "log",
        )
        .unwrap_err()
    }
}
