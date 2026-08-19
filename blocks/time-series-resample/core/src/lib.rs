//! gizza-ai/time-series-resample core — pure compute, shared by the chat skill
//! block and the web page. No wafer/wasm-bindgen deps.
//!
//! Buckets a timestamped CSV series into fixed calendar intervals (`5m`, `1h`,
//! `1d`, `1w`, `1mo`, `1q`, `1y`) and aggregates every numeric value column per
//! bucket (mean/sum/min/max/count/median/first/last/std/var/OHLC).
//!
//! Everything is computed in **UTC**: input timestamps carrying an offset are
//! converted to UTC first, and bucket edges are aligned to the UTC epoch
//! (weeks to Monday, months/quarters/years to the 1st). `origin` can re-anchor
//! a fixed-width grid to the first row instead, and `offset` shifts every edge
//! by a fixed amount, which is how you align to a non-UTC day boundary.
//!
//! Rows do not need to be sorted — buckets come back in chronological order.

use chrono::{DateTime, Datelike, NaiveDate, NaiveDateTime};

/// Hard input cap (~2 MB of CSV text).
const MAX_BYTES: usize = 2_000_000;
/// Hard cap on parsed data rows.
const MAX_ROWS: usize = 200_000;
/// Hard cap on emitted buckets (gap filling can generate a lot of them).
const MAX_OUT_ROWS: usize = 100_000;

const MS_PER_SEC: i64 = 1_000;
const MS_PER_MIN: i64 = 60 * MS_PER_SEC;
const MS_PER_HOUR: i64 = 60 * MS_PER_MIN;
const MS_PER_DAY: i64 = 24 * MS_PER_HOUR;
const MS_PER_WEEK: i64 = 7 * MS_PER_DAY;
/// 1970-01-01 was a Thursday; shifting the week anchor back 3 days starts
/// weeks on Monday.
const WEEK_ANCHOR: i64 = -3 * MS_PER_DAY;

/// A bucket width: either a fixed millisecond span (with the epoch anchor its
/// edges align to) or a whole number of calendar months.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Step {
    Fixed { width_ms: i64, anchor_ms: i64 },
    Months(i64),
}

#[derive(Clone, Copy, PartialEq)]
enum Agg {
    Mean,
    Sum,
    Min,
    Max,
    Count,
    Median,
    First,
    Last,
    Std,
    Var,
    Ohlc,
}

impl Agg {
    fn parse(s: &str) -> Result<Agg, String> {
        Ok(match s.trim().to_ascii_lowercase().as_str() {
            "" | "mean" | "avg" | "average" => Agg::Mean,
            "sum" | "total" => Agg::Sum,
            "min" => Agg::Min,
            "max" => Agg::Max,
            "count" => Agg::Count,
            "median" => Agg::Median,
            "first" => Agg::First,
            "last" => Agg::Last,
            "std" | "stdev" | "stddev" => Agg::Std,
            "var" | "variance" => Agg::Var,
            "ohlc" => Agg::Ohlc,
            other => {
                return Err(format!(
                    "aggregate must be one of mean/sum/min/max/count/median/first/last/std/var/ohlc, got '{other}'"
                ))
            }
        })
    }
    /// Output sub-column suffixes for one source column.
    fn suffixes(self) -> &'static [&'static str] {
        if self == Agg::Ohlc {
            &["_open", "_high", "_low", "_close"]
        } else {
            &[""]
        }
    }
    /// Aggregate one bucket's values (already in chronological order).
    fn apply(self, vals: &[f64]) -> Vec<Option<f64>> {
        if vals.is_empty() {
            return match self {
                Agg::Count => vec![Some(0.0)],
                Agg::Ohlc => vec![None; 4],
                _ => vec![None],
            };
        }
        match self {
            Agg::Mean => vec![Some(vals.iter().sum::<f64>() / vals.len() as f64)],
            Agg::Sum => vec![Some(vals.iter().sum::<f64>())],
            Agg::Min => vec![Some(vals.iter().cloned().fold(f64::INFINITY, f64::min))],
            Agg::Max => vec![Some(vals.iter().cloned().fold(f64::NEG_INFINITY, f64::max))],
            Agg::Count => vec![Some(vals.len() as f64)],
            Agg::First => vec![Some(vals[0])],
            Agg::Last => vec![Some(vals[vals.len() - 1])],
            // Sample (n-1) variance, matching the spreadsheet/pandas default;
            // a single-value bucket has no sample spread, so it stays blank.
            Agg::Std | Agg::Var => {
                if vals.len() < 2 {
                    return vec![None];
                }
                let mean = vals.iter().sum::<f64>() / vals.len() as f64;
                let var = vals.iter().map(|v| (v - mean) * (v - mean)).sum::<f64>()
                    / (vals.len() - 1) as f64;
                vec![Some(if self == Agg::Std { var.sqrt() } else { var })]
            }
            Agg::Median => {
                let mut s = vals.to_vec();
                s.sort_by(|a, b| a.partial_cmp(b).unwrap());
                let n = s.len();
                vec![Some(if n % 2 == 1 {
                    s[n / 2]
                } else {
                    (s[n / 2 - 1] + s[n / 2]) / 2.0
                })]
            }
            Agg::Ohlc => vec![
                Some(vals[0]),
                Some(vals.iter().cloned().fold(f64::NEG_INFINITY, f64::max)),
                Some(vals.iter().cloned().fold(f64::INFINITY, f64::min)),
                Some(vals[vals.len() - 1]),
            ],
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Fill {
    Skip,
    Empty,
    Zero,
    Previous,
    Linear,
}

impl Fill {
    fn parse(s: &str) -> Result<Fill, String> {
        Ok(match s.trim().to_ascii_lowercase().as_str() {
            "" | "skip" | "none" | "drop" => Fill::Skip,
            "empty" | "blank" | "null" => Fill::Empty,
            "zero" | "0" => Fill::Zero,
            "previous" | "ffill" | "hold" => Fill::Previous,
            "linear" | "interpolate" => Fill::Linear,
            other => {
                return Err(format!(
                    "fill must be one of skip/empty/zero/previous/linear, got '{other}'"
                ))
            }
        })
    }
}

/// Split one CSV line on `delim`, honouring `"` quoting with `""` escapes.
fn split_line(line: &str, delim: char) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_q = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if in_q {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    cur.push('"');
                } else {
                    in_q = false;
                }
            } else {
                cur.push(c);
            }
        } else if c == '"' && cur.trim().is_empty() {
            cur.clear();
            in_q = true;
        } else if c == delim {
            out.push(cur.trim().to_string());
            cur = String::new();
        } else {
            cur.push(c);
        }
    }
    out.push(cur.trim().to_string());
    out
}

/// Pick the delimiter by counting candidates outside quotes on the first line.
fn detect_delim(first_line: &str) -> char {
    let mut best = (',', 0usize);
    for cand in [',', '\t', ';', '|'] {
        let mut in_q = false;
        let mut n = 0usize;
        for c in first_line.chars() {
            if c == '"' {
                in_q = !in_q;
            } else if c == cand && !in_q {
                n += 1;
            }
        }
        if n > best.1 {
            best = (cand, n);
        }
    }
    best.0
}

fn csv_escape(s: &str, delim: char) -> String {
    if s.contains(delim) || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// Parse a numeric cell, rejecting non-finite so `NaN`/`inf` text can't mark a
/// text column as numeric.
fn parse_num(s: &str) -> Option<f64> {
    let t = s.trim();
    if t.is_empty() {
        return None;
    }
    t.parse::<f64>().ok().filter(|v| v.is_finite())
}

/// Parse a timestamp cell to milliseconds since the Unix epoch (UTC).
///
/// Accepts RFC-3339 / ISO-8601 (with or without an offset, `T` or space
/// separated), plain `YYYY-MM-DD`, `YYYY/MM/DD` variants, and bare epoch
/// numbers (>= 1e11 is read as milliseconds, otherwise as seconds).
pub fn parse_timestamp(s: &str) -> Option<i64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let numeric = s
        .chars()
        .all(|c| c.is_ascii_digit() || c == '-' || c == '+' || c == '.' || c == 'e' || c == 'E');
    if numeric && !s.contains('-') || (numeric && s.starts_with('-') && !s[1..].contains('-')) {
        if let Some(n) = parse_num(s) {
            return Some(if n.abs() >= 1e11 {
                n.round() as i64
            } else {
                (n * 1000.0).round() as i64
            });
        }
    }
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt.timestamp_millis());
    }
    let norm = s.replacen(' ', "T", 1).replace('/', "-");
    if let Ok(dt) = DateTime::parse_from_rfc3339(&norm) {
        return Some(dt.timestamp_millis());
    }
    let naive = norm.trim_end_matches('Z');
    for f in [
        "%Y-%m-%dT%H:%M:%S%.f",
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%dT%H:%M",
        "%d-%m-%YT%H:%M:%S",
    ] {
        if let Ok(dt) = NaiveDateTime::parse_from_str(naive, f) {
            return Some(dt.and_utc().timestamp_millis());
        }
    }
    for f in ["%Y-%m-%d", "%d-%m-%Y"] {
        if let Ok(d) = NaiveDate::parse_from_str(naive, f) {
            return Some(d.and_hms_opt(0, 0, 0)?.and_utc().timestamp_millis());
        }
    }
    None
}

/// Split `12h` into (12, "h"); a bare unit (`h`) means 1.
fn split_qty_unit(s: &str) -> Result<(i64, String), String> {
    let t = s.trim().to_ascii_lowercase();
    let sign = if t.starts_with('-') { -1i64 } else { 1i64 };
    let t = t.trim_start_matches(['-', '+']);
    let digits: String = t.chars().take_while(|c| c.is_ascii_digit()).collect();
    let unit: String = t.chars().skip(digits.len()).collect();
    let unit = unit.trim().to_string();
    let n = if digits.is_empty() {
        1
    } else {
        digits
            .parse::<i64>()
            .map_err(|_| format!("'{s}' has a number that is too large"))?
    };
    if unit.is_empty() {
        return Err(format!(
            "'{s}' is missing a unit — use ms, s, m (minute), h, d, w, mo (month), q or y, e.g. 15m or 1d"
        ));
    }
    Ok((sign * n, unit))
}

/// Parse a bucket width like `15m`, `1h`, `1d`, `1w`, `1mo`, `1q`, `1y`.
fn parse_interval(s: &str) -> Result<Step, String> {
    let raw = s.trim();
    if raw.is_empty() {
        return Ok(Step::Fixed {
            width_ms: MS_PER_HOUR,
            anchor_ms: 0,
        });
    }
    let (n, unit) = split_qty_unit(raw)?;
    if n <= 0 {
        return Err(format!(
            "interval must be a positive multiple of a unit, got '{raw}'"
        ));
    }
    let step = match unit.as_str() {
        "ms" | "milli" | "millis" | "millisecond" | "milliseconds" => Step::Fixed {
            width_ms: n,
            anchor_ms: 0,
        },
        "s" | "sec" | "secs" | "second" | "seconds" => Step::Fixed {
            width_ms: n * MS_PER_SEC,
            anchor_ms: 0,
        },
        "m" | "min" | "mins" | "minute" | "minutes" => Step::Fixed {
            width_ms: n * MS_PER_MIN,
            anchor_ms: 0,
        },
        "h" | "hr" | "hrs" | "hour" | "hours" => Step::Fixed {
            width_ms: n * MS_PER_HOUR,
            anchor_ms: 0,
        },
        "d" | "day" | "days" => Step::Fixed {
            width_ms: n * MS_PER_DAY,
            anchor_ms: 0,
        },
        "w" | "wk" | "wks" | "week" | "weeks" => Step::Fixed {
            width_ms: n * MS_PER_WEEK,
            anchor_ms: WEEK_ANCHOR,
        },
        "mo" | "mon" | "mth" | "month" | "months" => Step::Months(n),
        "q" | "qtr" | "quarter" | "quarters" => Step::Months(n * 3),
        "y" | "yr" | "yrs" | "year" | "years" => Step::Months(n * 12),
        other => {
            return Err(format!(
                "unknown interval unit '{other}' in '{raw}' — use ms, s, m (minute), h, d, w, mo (month), q or y"
            ))
        }
    };
    Ok(step)
}

/// Parse the boundary shift (`30m`, `-3d`, blank = none). Calendar units are
/// rejected here: a shift has to be an exact duration.
fn parse_offset(s: &str) -> Result<i64, String> {
    let raw = s.trim();
    if raw.is_empty() {
        return Ok(0);
    }
    let (n, unit) = split_qty_unit(raw)?;
    Ok(match unit.as_str() {
        "ms" | "milli" | "millis" | "millisecond" | "milliseconds" => n,
        "s" | "sec" | "secs" | "second" | "seconds" => n * MS_PER_SEC,
        "m" | "min" | "mins" | "minute" | "minutes" => n * MS_PER_MIN,
        "h" | "hr" | "hrs" | "hour" | "hours" => n * MS_PER_HOUR,
        "d" | "day" | "days" => n * MS_PER_DAY,
        "w" | "wk" | "wks" | "week" | "weeks" => n * MS_PER_WEEK,
        other => {
            return Err(format!(
                "offset unit '{other}' is not a fixed duration — use ms, s, m, h, d or w (e.g. -3d or 30m)"
            ))
        }
    })
}

/// Which instant the bucket grid is anchored to.
#[derive(Clone, Copy, PartialEq)]
enum Origin {
    /// Unix epoch (weeks start Monday, months/quarters/years on the 1st).
    Epoch,
    /// The earliest timestamp in the data.
    Start,
    /// UTC midnight of the earliest timestamp's day.
    StartDay,
}

impl Origin {
    fn parse(s: &str) -> Result<Origin, String> {
        Ok(match s.trim().to_ascii_lowercase().as_str() {
            "" | "epoch" | "unix" => Origin::Epoch,
            "start" | "first" => Origin::Start,
            "start_day" | "start-day" | "startday" | "midnight" => Origin::StartDay,
            other => {
                return Err(format!(
                    "origin must be one of epoch/start/start_day, got '{other}'"
                ))
            }
        })
    }
}

/// Extra edge shift that puts the chosen origin exactly on a bucket edge.
/// Calendar steps (months/quarters/years) always start on the 1st, so — like
/// every reference implementation — origin only applies to fixed widths.
fn origin_shift(origin: Origin, step: Step, first_ts: i64) -> i64 {
    let (width_ms, anchor_ms) = match step {
        Step::Fixed {
            width_ms,
            anchor_ms,
        } => (width_ms, anchor_ms),
        Step::Months(_) => return 0,
    };
    let base = match origin {
        Origin::Epoch => return 0,
        Origin::Start => first_ts,
        Origin::StartDay => first_ts.div_euclid(MS_PER_DAY) * MS_PER_DAY,
    };
    (base - anchor_ms).rem_euclid(width_ms)
}

fn months_to_ms(total_months: i64) -> Option<i64> {
    let y = total_months.div_euclid(12);
    let m = total_months.rem_euclid(12) + 1;
    let d = NaiveDate::from_ymd_opt(i32::try_from(y).ok()?, m as u32, 1)?;
    Some(d.and_hms_opt(0, 0, 0)?.and_utc().timestamp_millis())
}

fn ms_to_total_months(ms: i64) -> Option<i64> {
    let dt = DateTime::from_timestamp_millis(ms)?;
    Some(dt.year() as i64 * 12 + (dt.month0() as i64))
}

/// Start of the bucket `ts` falls in. `closed_right` makes an exact boundary
/// hit belong to the preceding bucket, i.e. buckets are `(start, end]`.
fn bucket_start(ts: i64, step: Step, offset_ms: i64, closed_right: bool) -> Result<i64, String> {
    match step {
        Step::Fixed {
            width_ms,
            anchor_ms,
        } => {
            let t = ts - offset_ms - anchor_ms;
            let mut b = t.div_euclid(width_ms) * width_ms;
            if closed_right && t == b {
                b -= width_ms;
            }
            Ok(b + anchor_ms + offset_ms)
        }
        Step::Months(n) => {
            let t = ts - offset_ms;
            let total = ms_to_total_months(t)
                .ok_or_else(|| "a timestamp is outside the supported date range".to_string())?;
            let mut start_total = total.div_euclid(n) * n;
            let mut start = months_to_ms(start_total)
                .ok_or_else(|| "a timestamp is outside the supported date range".to_string())?;
            if closed_right && t == start {
                start_total -= n;
                start = months_to_ms(start_total)
                    .ok_or_else(|| "a timestamp is outside the supported date range".to_string())?;
            }
            Ok(start + offset_ms)
        }
    }
}

/// The start of the bucket after the one starting at `start`.
fn next_bucket(start: i64, step: Step, offset_ms: i64) -> Result<i64, String> {
    match step {
        Step::Fixed { width_ms, .. } => Ok(start + width_ms),
        Step::Months(n) => {
            let total = ms_to_total_months(start - offset_ms)
                .ok_or_else(|| "a bucket edge is outside the supported date range".to_string())?;
            months_to_ms(total + n)
                .map(|m| m + offset_ms)
                .ok_or_else(|| "a bucket edge is outside the supported date range".to_string())
        }
    }
}

/// Render a number without float noise (rounded to 10 decimals, trimmed).
fn fmt_num(v: f64) -> String {
    if !v.is_finite() {
        return String::new();
    }
    let r = if v.abs() < 1e15 {
        (v * 1e10).round() / 1e10
    } else {
        v
    };
    let s = format!("{r}");
    if s == "-0" {
        "0".to_string()
    } else {
        s
    }
}

fn fmt_time(ms: i64, time_format: &str) -> Result<String, String> {
    let dt = DateTime::from_timestamp_millis(ms)
        .ok_or_else(|| "a bucket timestamp is outside the supported date range".to_string())?;
    Ok(match time_format.trim().to_ascii_lowercase().as_str() {
        "" | "iso" | "iso8601" | "rfc3339" => {
            if ms.rem_euclid(1000) == 0 {
                dt.format("%Y-%m-%dT%H:%M:%SZ").to_string()
            } else {
                dt.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
            }
        }
        "date" => dt.format("%Y-%m-%d").to_string(),
        "datetime" => dt.format("%Y-%m-%d %H:%M:%S").to_string(),
        "epoch_seconds" | "epoch" | "seconds" => {
            if ms.rem_euclid(1000) == 0 {
                format!("{}", ms / 1000)
            } else {
                fmt_num(ms as f64 / 1000.0)
            }
        }
        "epoch_millis" | "millis" | "ms" => format!("{ms}"),
        other => {
            return Err(format!(
                "time_format must be one of iso/date/datetime/epoch_seconds/epoch_millis, got '{other}'"
            ))
        }
    })
}

/// Resolve one column spec (header name, case-insensitive name, or 1-based
/// column number) to a column index.
fn resolve_col(spec: &str, names: &[String]) -> Result<usize, String> {
    let s = spec.trim();
    if let Some(i) = names.iter().position(|n| n == s) {
        return Ok(i);
    }
    if let Ok(n) = s.parse::<usize>() {
        if n >= 1 && n <= names.len() {
            return Ok(n - 1);
        }
        return Err(format!(
            "column number {n} is out of range — the input has {} columns",
            names.len()
        ));
    }
    let lower = s.to_ascii_lowercase();
    if let Some(i) = names.iter().position(|n| n.to_ascii_lowercase() == lower) {
        return Ok(i);
    }
    Err(format!(
        "column '{s}' not found — available columns: {}",
        names.join(", ")
    ))
}

struct Row {
    ts: i64,
    cells: Vec<String>,
}

/// Resample a timestamped CSV series to a coarser interval.
#[allow(clippy::too_many_arguments)]
pub fn resample(
    data: &str,
    time_column: &str,
    value_columns: &str,
    interval: &str,
    aggregate: &str,
    label: &str,
    closed: &str,
    fill: &str,
    origin: &str,
    offset: &str,
    time_format: &str,
    output: &str,
) -> Result<String, String> {
    if data.len() > MAX_BYTES {
        return Err(format!(
            "input is {} bytes; the cap is {MAX_BYTES} bytes (~2 MB) — split the series or drop unused columns",
            data.len()
        ));
    }
    let trimmed = data.trim_matches(|c| c == '\n' || c == '\r' || c == ' ');
    if trimmed.is_empty() {
        return Err("data is empty — paste a CSV with a timestamp column and at least one numeric column".into());
    }

    let step = parse_interval(interval)?;
    let agg = Agg::parse(aggregate)?;
    let fill = Fill::parse(fill)?;
    let origin = Origin::parse(origin)?;
    let offset_ms = parse_offset(offset)?;
    let label_end = match label.trim().to_ascii_lowercase().as_str() {
        "" | "start" | "left" => false,
        "end" | "right" => true,
        other => {
            return Err(format!(
                "label must be 'start' or 'end', got '{other}'"
            ))
        }
    };
    let closed_right = match closed.trim().to_ascii_lowercase().as_str() {
        "" | "left" => false,
        "right" => true,
        other => {
            return Err(format!(
                "closed must be 'left' or 'right', got '{other}'"
            ))
        }
    };
    let out_json = match output.trim().to_ascii_lowercase().as_str() {
        "" | "csv" => false,
        "json" => true,
        other => {
            return Err(format!("output must be 'csv' or 'json', got '{other}'"))
        }
    };
    // Validated up front so a bad value fails before the (slower) parse.
    fmt_time(0, time_format)?;

    // ---- split into cells -------------------------------------------------
    let lines: Vec<&str> = trimmed
        .lines()
        .map(|l| l.trim_end_matches('\r'))
        .filter(|l| !l.trim().is_empty())
        .collect();
    let delim = detect_delim(lines[0]);
    let raw: Vec<Vec<String>> = lines.iter().map(|l| split_line(l, delim)).collect();
    let width = raw.iter().map(|r| r.len()).max().unwrap_or(1);
    if width < 2 {
        return Err(format!(
            "need at least 2 columns (a timestamp and a value) — line 1 has 1 column; the detected delimiter was '{}'",
            if delim == '\t' { "tab".to_string() } else { delim.to_string() }
        ));
    }

    // A header row is any first row that is not entirely timestamps/numbers;
    // asking for a value column by NAME also implies one.
    let named_request = |s: &str| {
        let t = s.trim();
        !t.is_empty() && t.parse::<usize>().is_err()
    };
    let first_all_data = raw[0]
        .iter()
        .all(|c| c.trim().is_empty() || parse_num(c).is_some() || parse_timestamp(c).is_some());
    let has_header = !first_all_data
        || named_request(time_column)
        || value_columns.split(',').any(named_request);

    let names: Vec<String> = if has_header {
        (0..width)
            .map(|i| {
                let n = raw[0].get(i).map(|s| s.trim()).unwrap_or("");
                if n.is_empty() {
                    format!("column{}", i + 1)
                } else {
                    n.to_string()
                }
            })
            .collect()
    } else {
        (0..width).map(|i| format!("column{}", i + 1)).collect()
    };
    let first_data = if has_header { 1 } else { 0 };
    if raw.len() <= first_data {
        return Err("no data rows found — the input has only a header".into());
    }
    if raw.len() - first_data > MAX_ROWS {
        return Err(format!(
            "input has {} data rows; the cap is {MAX_ROWS}",
            raw.len() - first_data
        ));
    }

    // ---- pick columns -----------------------------------------------------
    let t_idx = if time_column.trim().is_empty() {
        0
    } else {
        resolve_col(time_column, &names)?
    };

    let numeric_col = |idx: usize| -> bool {
        raw[first_data..].iter().all(|r| {
            let c = r.get(idx).map(|s| s.trim()).unwrap_or("");
            c.is_empty() || parse_num(c).is_some()
        })
    };

    let v_idx: Vec<usize> = if value_columns.trim().is_empty() {
        let picked: Vec<usize> = (0..width).filter(|i| *i != t_idx && numeric_col(*i)).collect();
        if picked.is_empty() {
            return Err(format!(
                "no numeric value columns found next to the timestamp column '{}' — name one with value_columns, or check for stray text in the data",
                names[t_idx]
            ));
        }
        picked
    } else {
        let mut picked = Vec::new();
        for spec in value_columns.split(',').filter(|s| !s.trim().is_empty()) {
            let i = resolve_col(spec, &names)?;
            if i == t_idx {
                return Err(format!(
                    "'{}' is the timestamp column — it cannot also be a value column",
                    names[i]
                ));
            }
            if !numeric_col(i) {
                let bad = raw[first_data..]
                    .iter()
                    .enumerate()
                    .find(|(_, r)| {
                        let c = r.get(i).map(|s| s.trim()).unwrap_or("");
                        !c.is_empty() && parse_num(c).is_none()
                    })
                    .map(|(n, r)| (n + first_data + 1, r[i].clone()));
                let (line, cell) = bad.unwrap_or((0, String::new()));
                return Err(format!(
                    "value column '{}' is not numeric — line {line} has '{cell}', expected a number or a blank cell",
                    names[i]
                ));
            }
            if !picked.contains(&i) {
                picked.push(i);
            }
        }
        if picked.is_empty() {
            return Err("value_columns listed no usable columns — leave it blank to use every numeric column".into());
        }
        picked
    };

    // ---- parse rows -------------------------------------------------------
    let mut rows: Vec<Row> = Vec::with_capacity(raw.len() - first_data);
    for (n, r) in raw[first_data..].iter().enumerate() {
        let cell = r.get(t_idx).map(|s| s.trim()).unwrap_or("");
        if cell.is_empty() {
            continue;
        }
        let ts = parse_timestamp(cell).ok_or_else(|| {
            format!(
                "line {} has '{cell}' in the timestamp column '{}' — expected an ISO-8601 date/time (2024-05-01T13:20:00Z, 2024-05-01 13:20, 2024-05-01) or an epoch number",
                n + first_data + 1,
                names[t_idx]
            )
        })?;
        rows.push(Row {
            ts,
            cells: r.clone(),
        });
    }
    if rows.is_empty() {
        return Err("no rows had a timestamp — every cell in the timestamp column was blank".into());
    }
    rows.sort_by_key(|r| r.ts);
    // The origin only shifts the grid, so it folds into the offset.
    let offset_ms = offset_ms + origin_shift(origin, step, rows[0].ts);

    // ---- bucket -----------------------------------------------------------
    // rows are sorted, so bucket starts come out ascending and grouping is a
    // single pass; values keep chronological order for first/last/OHLC.
    let mut starts: Vec<i64> = Vec::new();
    let mut per_bucket: Vec<Vec<Vec<f64>>> = Vec::new();
    for row in &rows {
        let start = bucket_start(row.ts, step, offset_ms, closed_right)?;
        if starts.last() != Some(&start) {
            starts.push(start);
            per_bucket.push(vec![Vec::new(); v_idx.len()]);
            if starts.len() > MAX_OUT_ROWS {
                return Err(format!(
                    "that interval produces more than {MAX_OUT_ROWS} buckets — use a coarser interval"
                ));
            }
        }
        let slot = per_bucket.last_mut().unwrap();
        for (k, &ci) in v_idx.iter().enumerate() {
            if let Some(v) = row.cells.get(ci).and_then(|c| parse_num(c)) {
                slot[k].push(v);
            }
        }
    }

    let n_sub = agg.suffixes().len();
    // values[bucket][column][sub]
    let mut times: Vec<i64> = Vec::new();
    let mut values: Vec<Vec<Vec<Option<f64>>>> = Vec::new();

    if fill == Fill::Skip {
        for (b, start) in starts.iter().enumerate() {
            times.push(*start);
            values.push(per_bucket[b].iter().map(|v| agg.apply(v)).collect());
        }
    } else {
        let last = *starts.last().unwrap();
        let mut cur = starts[0];
        let mut b = 0usize;
        loop {
            times.push(cur);
            if b < starts.len() && starts[b] == cur {
                values.push(per_bucket[b].iter().map(|v| agg.apply(v)).collect());
                b += 1;
            } else {
                values.push(vec![vec![None; n_sub]; v_idx.len()]);
            }
            if cur >= last {
                break;
            }
            cur = next_bucket(cur, step, offset_ms)?;
            if times.len() > MAX_OUT_ROWS {
                return Err(format!(
                    "filling gaps at that interval produces more than {MAX_OUT_ROWS} rows — use a coarser interval or fill=skip"
                ));
            }
        }
        apply_fill(&mut values, fill);
    }

    // ---- render -----------------------------------------------------------
    let time_name = if has_header {
        names[t_idx].clone()
    } else {
        "time".to_string()
    };
    let mut headers: Vec<String> = vec![time_name];
    for &ci in &v_idx {
        for suf in agg.suffixes() {
            headers.push(format!("{}{suf}", names[ci]));
        }
    }

    let time_of = |b: usize, times: &Vec<i64>| -> Result<i64, String> {
        if label_end {
            next_bucket(times[b], step, offset_ms)
        } else {
            Ok(times[b])
        }
    };

    if out_json {
        let mut items = Vec::with_capacity(times.len());
        for b in 0..times.len() {
            let mut obj = serde_json::Map::new();
            obj.insert(
                headers[0].clone(),
                serde_json::Value::String(fmt_time(time_of(b, &times)?, time_format)?),
            );
            let mut h = 1usize;
            for col in &values[b] {
                for sub in col {
                    obj.insert(
                        headers[h].clone(),
                        match sub {
                            Some(v) => serde_json::Number::from_f64(
                                fmt_num(*v).parse::<f64>().unwrap_or(*v),
                            )
                            .map(serde_json::Value::Number)
                            .unwrap_or(serde_json::Value::Null),
                            None => serde_json::Value::Null,
                        },
                    );
                    h += 1;
                }
            }
            items.push(serde_json::Value::Object(obj));
        }
        return serde_json::to_string_pretty(&serde_json::Value::Array(items))
            .map_err(|e| format!("could not serialise JSON output: {e}"));
    }

    let mut out = String::new();
    out.push_str(
        &headers
            .iter()
            .map(|h| csv_escape(h, delim))
            .collect::<Vec<_>>()
            .join(&delim.to_string()),
    );
    for b in 0..times.len() {
        out.push('\n');
        let mut cells: Vec<String> = vec![csv_escape(&fmt_time(time_of(b, &times)?, time_format)?, delim)];
        for col in &values[b] {
            for sub in col {
                cells.push(sub.map(fmt_num).unwrap_or_default());
            }
        }
        out.push_str(&cells.join(&delim.to_string()));
    }
    Ok(out)
}

/// Replace the `None` slots produced by empty buckets per the fill policy.
/// `Empty` is a no-op (blank cells are the representation).
fn apply_fill(values: &mut [Vec<Vec<Option<f64>>>], fill: Fill) {
    if values.is_empty() || fill == Fill::Empty {
        return;
    }
    let n_col = values[0].len();
    let n_sub = values[0][0].len();
    for c in 0..n_col {
        for s in 0..n_sub {
            match fill {
                Fill::Zero => {
                    for row in values.iter_mut() {
                        if row[c][s].is_none() {
                            row[c][s] = Some(0.0);
                        }
                    }
                }
                Fill::Previous => {
                    let mut prev: Option<f64> = None;
                    for row in values.iter_mut() {
                        match row[c][s] {
                            Some(v) => prev = Some(v),
                            None => row[c][s] = prev,
                        }
                    }
                }
                Fill::Linear => {
                    let mut i = 0usize;
                    while i < values.len() {
                        if values[i][c][s].is_some() {
                            i += 1;
                            continue;
                        }
                        let gap_start = i;
                        while i < values.len() && values[i][c][s].is_none() {
                            i += 1;
                        }
                        let before = if gap_start == 0 {
                            None
                        } else {
                            values[gap_start - 1][c][s]
                        };
                        let after = if i < values.len() { values[i][c][s] } else { None };
                        if let (Some(a), Some(b)) = (before, after) {
                            let steps = (i - gap_start + 1) as f64;
                            for (k, g) in (gap_start..i).enumerate() {
                                values[g][c][s] = Some(a + (b - a) * ((k + 1) as f64 / steps));
                            }
                        }
                    }
                }
                Fill::Skip | Fill::Empty => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEF: (&str, &str, &str, &str, &str, &str) = ("mean", "start", "left", "skip", "", "iso");

    fn run(data: &str, interval: &str) -> Result<String, String> {
        resample(data, "", "", interval, DEF.0, DEF.1, DEF.2, DEF.3, "", DEF.4, DEF.5, "csv")
    }

    #[test]
    fn hourly_mean_from_minute_data() {
        let csv = "time,temp\n2024-05-01T10:00:00Z,10\n2024-05-01T10:30:00Z,20\n2024-05-01T11:15:00Z,30";
        assert_eq!(
            run(csv, "1h").unwrap(),
            "time,temp\n2024-05-01T10:00:00Z,15\n2024-05-01T11:00:00Z,30"
        );
    }

    #[test]
    fn sum_with_multiple_value_columns_and_date_labels() {
        let csv = "day,a,b\n2024-01-01,1,10\n2024-01-02,2,20\n2024-01-08,4,40";
        let got = resample(csv, "", "", "1w", "sum", "start", "left", "skip", "", "", "date", "csv").unwrap();
        // 2024-01-01 is a Monday, so week 1 holds Jan 1 + Jan 2.
        assert_eq!(got, "day,a,b\n2024-01-01,3,30\n2024-01-08,4,40");
    }

    #[test]
    fn ohlc_expands_into_four_columns() {
        let csv = "t,px\n2024-03-04T09:00:00Z,10\n2024-03-04T09:20:00Z,14\n2024-03-04T09:40:00Z,9\n2024-03-04T09:50:00Z,12";
        let got = resample(csv, "", "", "1h", "ohlc", "start", "left", "skip", "", "", "iso", "csv").unwrap();
        assert_eq!(
            got,
            "t,px_open,px_high,px_low,px_close\n2024-03-04T09:00:00Z,10,14,9,12"
        );
    }

    #[test]
    fn gap_filling_and_label_end() {
        let csv = "t,v\n2024-01-01T00:00:00Z,4\n2024-01-01T03:00:00Z,8";
        let got = resample(csv, "", "", "1h", "mean", "end", "left", "linear", "", "", "iso", "csv").unwrap();
        assert_eq!(
            got,
            "t,v\n2024-01-01T01:00:00Z,4\n2024-01-01T02:00:00Z,5.3333333333\n2024-01-01T03:00:00Z,6.6666666667\n2024-01-01T04:00:00Z,8"
        );
    }

    #[test]
    fn closed_right_moves_boundary_rows_into_the_previous_bucket() {
        let csv = "t,v\n2024-01-01T00:00:00Z,1\n2024-01-01T01:00:00Z,2";
        let got = resample(csv, "", "", "1h", "sum", "start", "right", "skip", "", "", "iso", "csv").unwrap();
        assert_eq!(
            got,
            "t,v\n2023-12-31T23:00:00Z,1\n2024-01-01T00:00:00Z,2"
        );
    }

    #[test]
    fn offset_shifts_every_bucket_edge() {
        let csv = "t,v\n2024-01-01T06:00:00Z,1\n2024-01-01T20:00:00Z,2";
        let got = resample(csv, "", "", "1d", "count", "start", "left", "skip", "", "-5h", "datetime", "csv").unwrap();
        assert_eq!(got, "t,v\n2023-12-31 19:00:00,1\n2024-01-01 19:00:00,1");
    }

    #[test]
    fn monthly_json_output_with_epoch_input() {
        let csv = "ts,v\n1704067200,3\n1704153600,5\n1706745600,7";
        let got = resample(csv, "", "", "1mo", "sum", "start", "left", "skip", "", "", "date", "json").unwrap();
        assert!(got.contains("\"2024-01-01\""), "got {got}");
        assert!(got.contains("\"v\": 8"), "got {got}");
        assert!(got.contains("\"2024-02-01\""), "got {got}");
    }

    #[test]
    fn unsorted_rows_and_blank_cells_are_handled() {
        let csv = "t,v\n2024-01-02T00:00:00Z,5\n2024-01-01T00:00:00Z,\n2024-01-01T12:00:00Z,3";
        let got = resample(csv, "", "", "1d", "count", "start", "left", "skip", "", "", "date", "csv").unwrap();
        assert_eq!(got, "t,v\n2024-01-01,1\n2024-01-02,1");
    }

    #[test]
    fn tab_delimited_input_keeps_its_delimiter() {
        let csv = "t\tv\n2024-01-01T00:00:00Z\t1\n2024-01-01T00:00:30Z\t3";
        let got = resample(csv, "", "", "1m", "max", "start", "left", "skip", "", "", "iso", "csv").unwrap();
        assert_eq!(got, "t\tv\n2024-01-01T00:00:00Z\t3");
    }

    #[test]
    fn headerless_input_uses_positional_columns() {
        let csv = "2024-01-01T00:00:00Z,2\n2024-01-01T00:00:00Z,4";
        let got = resample(csv, "", "", "1h", "mean", "start", "left", "skip", "", "", "iso", "csv").unwrap();
        assert_eq!(got, "time,column2\n2024-01-01T00:00:00Z,3");
    }

    #[test]
    fn origin_start_anchors_the_grid_to_the_first_row() {
        let csv = "t,v\n2024-01-01T10:20:00Z,1\n2024-01-01T11:19:00Z,2\n2024-01-01T11:21:00Z,4";
        // epoch grid: 10:00 and 11:00 buckets.
        let epoch =
            resample(csv, "", "", "1h", "sum", "start", "left", "skip", "epoch", "", "iso", "csv")
                .unwrap();
        assert_eq!(
            epoch,
            "t,v\n2024-01-01T10:00:00Z,1\n2024-01-01T11:00:00Z,6"
        );
        // start grid: edges at 10:20, 11:20 — the 11:19 row joins the first.
        let start =
            resample(csv, "", "", "1h", "sum", "start", "left", "skip", "start", "", "iso", "csv")
                .unwrap();
        assert_eq!(
            start,
            "t,v\n2024-01-01T10:20:00Z,3\n2024-01-01T11:20:00Z,4"
        );
    }

    #[test]
    fn origin_start_day_anchors_to_midnight_of_the_first_row() {
        let csv = "t,v\n2024-01-01T05:00:00Z,1\n2024-01-01T12:00:00Z,2";
        // 7h buckets from midnight: 00:00-07:00 and 07:00-14:00.
        let got = resample(
            csv, "", "", "7h", "sum", "start", "left", "skip", "start_day", "", "iso", "csv",
        )
        .unwrap();
        assert_eq!(got, "t,v\n2024-01-01T00:00:00Z,1\n2024-01-01T07:00:00Z,2");
    }

    #[test]
    fn origin_is_ignored_for_calendar_months() {
        let csv = "t,v\n2024-01-17T00:00:00Z,1\n2024-02-03T00:00:00Z,2";
        let got = resample(
            csv, "", "", "1mo", "sum", "start", "left", "skip", "start", "", "date", "csv",
        )
        .unwrap();
        assert_eq!(got, "t,v\n2024-01-01,1\n2024-02-01,2");
    }

    #[test]
    fn std_and_var_use_the_sample_denominator() {
        let csv = "t,v\n2024-01-01T00:00:00Z,2\n2024-01-01T00:10:00Z,4\n2024-01-01T00:20:00Z,4\n2024-01-01T00:30:00Z,6";
        // mean 4, sum of squared deviations 8, sample variance 8/3.
        let var =
            resample(csv, "", "", "1h", "var", "start", "left", "skip", "", "", "iso", "csv")
                .unwrap();
        assert_eq!(var, "t,v\n2024-01-01T00:00:00Z,2.6666666667");
        let std =
            resample(csv, "", "", "1h", "std", "start", "left", "skip", "", "", "iso", "csv")
                .unwrap();
        assert_eq!(std, "t,v\n2024-01-01T00:00:00Z,1.6329931619");
    }

    #[test]
    fn std_of_a_single_value_bucket_is_blank() {
        let csv = "t,v\n2024-01-01T00:00:00Z,5";
        let got = resample(csv, "", "", "1h", "std", "start", "left", "skip", "", "", "iso", "csv")
            .unwrap();
        assert_eq!(got, "t,v\n2024-01-01T00:00:00Z,");
    }

    #[test]
    fn upsampling_to_a_finer_interval_interpolates_between_rows() {
        let csv = "d,v\n2024-01-01,10\n2024-01-03,30";
        let got = resample(
            csv, "", "", "1d", "mean", "start", "left", "linear", "", "", "date", "csv",
        )
        .unwrap();
        assert_eq!(got, "d,v\n2024-01-01,10\n2024-01-02,20\n2024-01-03,30");
    }

    #[test]
    fn err_bad_origin() {
        let e = resample(
            "t,v\n2024-01-01,1", "", "", "1d", "mean", "start", "left", "skip", "noon", "", "iso",
            "csv",
        )
        .unwrap_err();
        assert!(e.contains("origin must be one of epoch/start/start_day"), "{e}");
    }

    #[test]
    fn err_bad_timestamp_names_the_line_and_column() {
        let csv = "when,v\n2024-01-01T00:00:00Z,1\nyesterday,2";
        let e = run(csv, "1h").unwrap_err();
        assert!(e.contains("line 3"), "{e}");
        assert!(e.contains("'when'"), "{e}");
    }

    #[test]
    fn err_unknown_interval_unit() {
        let e = run("t,v\n2024-01-01,1", "3fortnights").unwrap_err();
        assert!(e.contains("unknown interval unit"), "{e}");
    }

    #[test]
    fn err_unknown_column() {
        let csv = "t,v\n2024-01-01,1";
        let e = resample(csv, "nope", "", "1d", "mean", "start", "left", "skip", "", "", "iso", "csv")
            .unwrap_err();
        assert!(e.contains("available columns: t, v"), "{e}");
    }

    #[test]
    fn err_non_numeric_value_column() {
        let csv = "t,v\n2024-01-01,ok\n2024-01-02,2";
        let e = resample(csv, "t", "v", "1d", "mean", "start", "left", "skip", "", "", "iso", "csv")
            .unwrap_err();
        assert!(e.contains("not numeric"), "{e}");
        assert!(e.contains("line 2"), "{e}");
    }

    #[test]
    fn err_empty_input() {
        assert!(run("   ", "1h").unwrap_err().contains("data is empty"));
    }

    #[test]
    fn err_bad_aggregate_and_offset_unit() {
        assert!(resample("t,v\n2024-01-01,1", "", "", "1d", "p95", "start", "left", "skip", "", "", "iso", "csv")
            .unwrap_err()
            .contains("aggregate must be"));
        assert!(resample("t,v\n2024-01-01,1", "", "", "1d", "mean", "start", "left", "skip", "", "1mo", "iso", "csv")
            .unwrap_err()
            .contains("not a fixed duration"));
    }
}
