//! gizza-ai/churn-cohort-retention core — cohort retention / churn tables and
//! curves from raw signup + activity data.
//!
//! Users are grouped into cohorts by the calendar bucket (month, week or day)
//! of their signup date — taken from an optional signup/users table, or from
//! each user's first activity when no signup table is supplied. For every
//! cohort the tool counts the distinct users active in each following period
//! and reports retention (active / cohort size) or period-over-period churn,
//! plus a weighted average row and a text retention curve.
//!
//! Pure-Rust (`csv`, `serde_json`). No wafer/wasm-bindgen deps, no clock: the
//! analysis "as of" date is either supplied or derived from the data, so every
//! run is deterministic.

use std::collections::{BTreeMap, HashMap, HashSet};

use serde::Serialize;

/// Hard cap on data rows (activity or signups) — this is a paste-sized tool.
pub const MAX_ROWS: usize = 50_000;
/// Hard cap on distinct cohorts, to stop a daily granularity over years of data
/// from rendering an unreadable table.
pub const MAX_COHORTS: usize = 1_000;
/// Hard cap on follow-up periods.
pub const MAX_PERIODS: usize = 36;

// ---------------------------------------------------------------------------
// dates (deterministic, no chrono / no clock)
// ---------------------------------------------------------------------------

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct Date {
    pub y: i32,
    pub m: u32,
    pub d: u32,
}

/// Days since 1970-01-01 (Howard Hinnant's `days_from_civil`).
fn days_from_civil(y: i32, m: u32, d: u32) -> i64 {
    let y = (y as i64) - if m <= 2 { 1 } else { 0 };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400; // [0, 399]
    let mp = ((m as i64) + 9) % 12; // March = 0
    let doy = (153 * mp + 2) / 5 + (d as i64) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Inverse of [`days_from_civil`].
fn civil_from_days(z: i64) -> Date {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    Date {
        y: (y + if m <= 2 { 1 } else { 0 }) as i32,
        m: m as u32,
        d: d as u32,
    }
}

fn days_in_month(y: i32, m: u32) -> u32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 {
                29
            } else {
                28
            }
        }
        _ => 0,
    }
}

impl Date {
    fn epoch_days(&self) -> i64 {
        days_from_civil(self.y, self.m, self.d)
    }
    fn iso(&self) -> String {
        format!("{:04}-{:02}-{:02}", self.y, self.m, self.d)
    }
}

/// Parse a date deterministically. Accepted forms:
/// `YYYY-MM-DD`, `YYYY-MM-DD[T ]HH:MM[:SS…]`, `YYYY-MM` (day 1), `YYYY/MM/DD`,
/// `YYYYMMDD`, 10-digit Unix epoch seconds, 13-digit epoch milliseconds.
/// Slash/dot dates that start with a day or a month (`03/04/2024`) are REJECTED
/// rather than guessed — the day/month order is ambiguous.
pub fn parse_date(raw: &str, label: &str) -> Result<Date, String> {
    let s = raw.trim();
    if s.is_empty() {
        return Err(format!("{label} is empty; expected a date like 2024-01-15"));
    }
    if s.bytes().all(|b| b.is_ascii_digit()) {
        return match s.len() {
            8 => {
                let y = s[0..4].parse::<i32>().unwrap();
                let m = s[4..6].parse::<u32>().unwrap();
                let d = s[6..8].parse::<u32>().unwrap();
                check_ymd(y, m, d, label, s)
            }
            10 => Ok(civil_from_days(
                s.parse::<i64>().map_err(|_| bad(label, s))?.div_euclid(86_400),
            )),
            13 => Ok(civil_from_days(
                s.parse::<i64>()
                    .map_err(|_| bad(label, s))?
                    .div_euclid(86_400_000),
            )),
            _ => Err(format!(
                "{label} '{s}' is all digits but not a date: expected YYYYMMDD, \
                 10-digit epoch seconds, or 13-digit epoch milliseconds"
            )),
        };
    }
    // Cut a time part off: "2024-01-15T09:30:00Z" / "2024-01-15 09:30:00".
    let date_part = s
        .split(['T', 't', ' '])
        .next()
        .unwrap_or(s)
        .trim_end_matches(',');
    let sep = if date_part.contains('-') {
        '-'
    } else if date_part.contains('/') {
        '/'
    } else {
        return Err(bad(label, s));
    };
    let parts: Vec<&str> = date_part.split(sep).collect();
    if parts[0].len() != 4 || !parts[0].bytes().all(|b| b.is_ascii_digit()) {
        return Err(format!(
            "{label} '{s}' is ambiguous: day-first and month-first dates cannot be \
             told apart. Convert the column to ISO-8601 (YYYY-MM-DD) first"
        ));
    }
    let y = parts[0].parse::<i32>().map_err(|_| bad(label, s))?;
    let m = parts
        .get(1)
        .ok_or_else(|| bad(label, s))?
        .parse::<u32>()
        .map_err(|_| bad(label, s))?;
    let d = match parts.get(2) {
        Some(p) => p.parse::<u32>().map_err(|_| bad(label, s))?,
        None => 1,
    };
    check_ymd(y, m, d, label, s)
}

fn bad(label: &str, s: &str) -> String {
    format!("{label} '{s}' is not a date: expected YYYY-MM-DD, an ISO-8601 timestamp, or a Unix epoch")
}

fn check_ymd(y: i32, m: u32, d: u32, label: &str, s: &str) -> Result<Date, String> {
    if !(1..=12).contains(&m) {
        return Err(format!("{label} '{s}' has month {m}; expected 1-12"));
    }
    let dim = days_in_month(y, m);
    if d < 1 || d > dim {
        return Err(format!(
            "{label} '{s}' has day {d}; {y:04}-{m:02} has {dim} day(s)"
        ));
    }
    Ok(Date { y, m, d })
}

// ---------------------------------------------------------------------------
// granularity / buckets
// ---------------------------------------------------------------------------

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Gran {
    Month,
    Week,
    Day,
}

impl Gran {
    fn parse(s: &str) -> Result<Gran, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "month" | "monthly" => Ok(Gran::Month),
            "week" | "weekly" => Ok(Gran::Week),
            "day" | "daily" => Ok(Gran::Day),
            other => Err(format!(
                "granularity must be month, week, or day, got '{other}'"
            )),
        }
    }
    fn adjective(&self) -> &'static str {
        match self {
            Gran::Month => "monthly",
            Gran::Week => "weekly",
            Gran::Day => "daily",
        }
    }
    /// The bucket index a date falls into. Weeks start on Monday (1970-01-01
    /// was a Thursday, so `+3` aligns the epoch week to its Monday).
    fn bucket(&self, d: Date) -> i64 {
        match self {
            Gran::Month => (d.y as i64) * 12 + (d.m as i64 - 1),
            Gran::Week => (d.epoch_days() + 3).div_euclid(7),
            Gran::Day => d.epoch_days(),
        }
    }
    fn label(&self, b: i64) -> String {
        match self {
            Gran::Month => format!("{:04}-{:02}", b.div_euclid(12), b.rem_euclid(12) + 1),
            Gran::Week => civil_from_days(b * 7 - 3).iso(),
            Gran::Day => civil_from_days(b).iso(),
        }
    }
}

// ---------------------------------------------------------------------------
// options
// ---------------------------------------------------------------------------

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Metric {
    Retention,
    Churn,
}

impl Metric {
    fn parse(s: &str) -> Result<Metric, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "retention" => Ok(Metric::Retention),
            "churn" => Ok(Metric::Churn),
            other => Err(format!("metric must be retention or churn, got '{other}'")),
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Values {
    Percent,
    Count,
    Both,
}

impl Values {
    fn parse(s: &str) -> Result<Values, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "percent" => Ok(Values::Percent),
            "count" => Ok(Values::Count),
            "both" => Ok(Values::Both),
            other => Err(format!(
                "values must be percent, count, or both, got '{other}'"
            )),
        }
    }
}

fn delim_byte(d: &str) -> Result<u8, String> {
    Ok(match d.trim() {
        "" | "," | "comma" => b',',
        "\t" | "tab" | "\\t" => b'\t',
        ";" | "semicolon" => b';',
        "|" | "pipe" => b'|',
        other => {
            let b = other.as_bytes();
            if b.len() == 1 {
                b[0]
            } else {
                return Err(format!(
                    "delimiter must be a single char or tab/comma/semicolon/pipe, got '{other}'"
                ));
            }
        }
    })
}

// ---------------------------------------------------------------------------
// report model
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CohortRow {
    /// Cohort label — `YYYY-MM` for monthly, the Monday `YYYY-MM-DD` for weekly,
    /// the date for daily.
    pub cohort: String,
    /// Distinct users that signed up in this cohort.
    pub users: usize,
    /// Distinct active users per period (`None` where the cohort has not aged
    /// far enough to observe that period by the analysis date).
    pub active: Vec<Option<usize>>,
    /// Retention: active users this period. Churn: users lost since the
    /// previous period (negative when users came back).
    pub counts: Vec<Option<i64>>,
    /// Retention: active / cohort size, percent. Churn: lost / previous
    /// period's active users, percent.
    pub percent: Vec<Option<f64>>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AverageRow {
    pub users: usize,
    pub counts: Vec<Option<i64>>,
    pub percent: Vec<Option<f64>>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Report {
    pub metric: String,
    pub granularity: String,
    /// Number of follow-up periods reported (columns are P0..=periods).
    pub periods: usize,
    /// Analysis date: cells beyond it are unobservable, not zero.
    pub as_of: String,
    pub total_users: usize,
    pub cohorts: Vec<CohortRow>,
    /// Weighted average across the cohorts that can observe each period.
    pub average: AverageRow,
    /// Activity rows dated before the user's signup date (ignored).
    pub pre_signup_rows: usize,
    /// Users seen in the activity data but absent from the signup table (ignored).
    pub unknown_users: usize,
}

/// Round a 0..1 ratio to a percentage with 2 decimals.
fn pct(ratio: f64) -> f64 {
    (ratio * 10000.0).round() / 100.0
}

// ---------------------------------------------------------------------------
// parsing helpers
// ---------------------------------------------------------------------------

fn read_rows(data: &str, delim: u8, what: &str) -> Result<Vec<Vec<String>>, String> {
    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(delim)
        .has_headers(false)
        .flexible(true)
        .from_reader(data.as_bytes());
    let mut out: Vec<Vec<String>> = Vec::new();
    for rec in rdr.records() {
        let rec = rec.map_err(|e| format!("could not parse the {what} data: {e}"))?;
        let row: Vec<String> = rec.iter().map(|f| f.trim().to_string()).collect();
        if row.iter().all(|f| f.is_empty()) {
            continue;
        }
        out.push(row);
        if out.len() > MAX_ROWS {
            return Err(format!(
                "{what} data has more than {MAX_ROWS} rows; summarize or sample it first"
            ));
        }
    }
    if out.is_empty() {
        return Err(format!("the {what} data has no rows"));
    }
    Ok(out)
}

/// Column names for a table: the header row, or `1..N` placeholders.
fn header_names(rows: &[Vec<String>], has_header: bool) -> Vec<String> {
    if has_header {
        rows[0].clone()
    } else {
        let n = rows.iter().map(|r| r.len()).max().unwrap_or(0);
        (1..=n).map(|i| i.to_string()).collect()
    }
}

/// Resolve a column reference (header name, or 1-based index) to a 0-based index.
fn resolve_col(
    spec: &str,
    names: &[String],
    has_header: bool,
    default_idx: usize,
    label: &str,
) -> Result<usize, String> {
    let spec = spec.trim();
    if spec.is_empty() {
        if default_idx < names.len() {
            return Ok(default_idx);
        }
        return Err(format!(
            "{label} column not given and the data has only {} column(s)",
            names.len()
        ));
    }
    if let Ok(n) = spec.parse::<usize>() {
        if (1..=names.len()).contains(&n) {
            return Ok(n - 1);
        }
        return Err(format!(
            "{label} column index {n} is out of range (1..={})",
            names.len()
        ));
    }
    if !has_header {
        return Err(format!(
            "{label} column '{spec}' is a name but the first row is not a header; \
             use a 1-based column index or turn the header option on"
        ));
    }
    names
        .iter()
        .position(|c| c == spec)
        .ok_or_else(|| format!("{label} column '{spec}' not found in the header row"))
}

/// Same as [`resolve_col`] but falls back to `default_idx` when a NAME is not
/// found — used for the user column of the optional signup table, whose header
/// may differ from the activity table's.
fn resolve_col_lenient(spec: &str, names: &[String], has_header: bool, default_idx: usize) -> usize {
    let spec = spec.trim();
    if let Ok(n) = spec.parse::<usize>() {
        if (1..=names.len()).contains(&n) {
            return n - 1;
        }
    } else if has_header {
        if let Some(i) = names.iter().position(|c| c == spec) {
            return i;
        }
    }
    default_idx
}

fn cell(row: &[String], idx: usize) -> &str {
    row.get(idx).map(|s| s.as_str()).unwrap_or("")
}

// ---------------------------------------------------------------------------
// compute
// ---------------------------------------------------------------------------

/// Build the full cohort report.
#[allow(clippy::too_many_arguments)]
pub fn compute(
    data: &str,
    signups: &str,
    user_spec: &str,
    date_spec: &str,
    signup_date_spec: &str,
    granularity: &str,
    periods: f64,
    metric: &str,
    as_of_spec: &str,
    has_header: bool,
    delimiter: &str,
) -> Result<Report, String> {
    if data.trim().is_empty() {
        return Err("activity data is empty: paste rows of user id + activity date".into());
    }
    let gran = Gran::parse(granularity)?;
    let metric = Metric::parse(metric)?;
    let delim = delim_byte(delimiter)?;
    if !periods.is_finite() || periods.fract() != 0.0 {
        return Err(format!("periods must be a whole number, got {periods}"));
    }
    let periods = periods as i64;
    if !(1..=MAX_PERIODS as i64).contains(&periods) {
        return Err(format!(
            "periods must be between 1 and {MAX_PERIODS}, got {periods}"
        ));
    }
    let periods = periods as usize;

    // --- activity table -----------------------------------------------------
    let rows = read_rows(data, delim, "activity")?;
    let names = header_names(&rows, has_header);
    let u_idx = resolve_col(user_spec, &names, has_header, 0, "user")?;
    let d_idx = resolve_col(date_spec, &names, has_header, 1, "activity date")?;
    let body = if has_header { &rows[1..] } else { &rows[..] };
    if body.is_empty() {
        return Err("the activity data has a header row but no data rows".into());
    }

    // user -> the set of activity buckets they were active in
    let mut activity: HashMap<String, HashSet<i64>> = HashMap::new();
    let mut first_seen: HashMap<String, i64> = HashMap::new();
    let mut max_date: Option<Date> = None;
    for (i, row) in body.iter().enumerate() {
        let uid = cell(row, u_idx);
        if uid.is_empty() {
            continue;
        }
        let raw = cell(row, d_idx);
        if raw.is_empty() {
            continue;
        }
        let line = i + 1 + usize::from(has_header);
        let date = parse_date(raw, &format!("activity date on line {line}"))?;
        if max_date.is_none_or(|m| date > m) {
            max_date = Some(date);
        }
        let b = gran.bucket(date);
        activity.entry(uid.to_string()).or_default().insert(b);
        first_seen
            .entry(uid.to_string())
            .and_modify(|f| {
                if b < *f {
                    *f = b
                }
            })
            .or_insert(b);
    }
    if activity.is_empty() {
        return Err(
            "no usable activity rows: every row had an empty user id or activity date".into(),
        );
    }

    // --- signup table (optional) -------------------------------------------
    let mut cohort_of: HashMap<String, i64> = HashMap::new();
    let mut unknown_users = 0usize;
    if signups.trim().is_empty() {
        for (u, b) in &first_seen {
            cohort_of.insert(u.clone(), *b);
        }
    } else {
        let srows = read_rows(signups, delim, "signup")?;
        let snames = header_names(&srows, has_header);
        let su_idx = resolve_col_lenient(user_spec, &snames, has_header, 0);
        let sd_idx = resolve_col(signup_date_spec, &snames, has_header, 1, "signup date")?;
        let sbody = if has_header { &srows[1..] } else { &srows[..] };
        if sbody.is_empty() {
            return Err("the signup data has a header row but no data rows".into());
        }
        for (i, row) in sbody.iter().enumerate() {
            let uid = cell(row, su_idx);
            let raw = cell(row, sd_idx);
            if uid.is_empty() || raw.is_empty() {
                continue;
            }
            let line = i + 1 + usize::from(has_header);
            let date = parse_date(raw, &format!("signup date on line {line}"))?;
            if max_date.is_none_or(|m| date > m) {
                max_date = Some(date);
            }
            let b = gran.bucket(date);
            // Duplicate signup rows: keep the earliest.
            cohort_of
                .entry(uid.to_string())
                .and_modify(|f| {
                    if b < *f {
                        *f = b
                    }
                })
                .or_insert(b);
        }
        if cohort_of.is_empty() {
            return Err(
                "no usable signup rows: every row had an empty user id or signup date".into(),
            );
        }
        unknown_users = activity.keys().filter(|u| !cohort_of.contains_key(*u)).count();
    }

    // --- analysis date ------------------------------------------------------
    let as_of = if as_of_spec.trim().is_empty() {
        max_date.ok_or("could not determine an analysis date from the data")?
    } else {
        parse_date(as_of_spec, "as_of")?
    };
    let as_of_bucket = gran.bucket(as_of);

    // --- cohort assembly ----------------------------------------------------
    let mut members: BTreeMap<i64, Vec<String>> = BTreeMap::new();
    for (u, b) in &cohort_of {
        members.entry(*b).or_default().push(u.clone());
    }
    if members.len() > MAX_COHORTS {
        return Err(format!(
            "the data spans {} {} cohorts (max {MAX_COHORTS}); use a coarser granularity",
            members.len(),
            gran.adjective()
        ));
    }

    let mut pre_signup_rows = 0usize;
    let mut cohorts: Vec<CohortRow> = Vec::with_capacity(members.len());
    // active[period] summed over observable cohorts, and their sizes.
    let mut sum_active = vec![0usize; periods + 1];
    let mut sum_size = vec![0usize; periods + 1];
    let mut total_users = 0usize;

    for (cb, users) in &members {
        let size = users.len();
        total_users += size;
        let observable_upto = (as_of_bucket - cb).max(-1);
        let mut active: Vec<Option<usize>> = vec![None; periods + 1];
        for (p, slot) in active.iter_mut().enumerate() {
            if (p as i64) <= observable_upto {
                *slot = Some(0);
            }
        }
        for u in users {
            let Some(buckets) = activity.get(u) else {
                continue;
            };
            for b in buckets {
                let rel = b - cb;
                if rel < 0 {
                    pre_signup_rows += 1;
                    continue;
                }
                if rel as usize <= periods {
                    if let Some(slot) = active[rel as usize].as_mut() {
                        *slot += 1;
                    }
                }
            }
        }
        for p in 0..=periods {
            if let Some(a) = active[p] {
                sum_active[p] += a;
                sum_size[p] += size;
            }
        }
        let (counts, percent) = derive(metric, &active, size);
        cohorts.push(CohortRow {
            cohort: gran.label(*cb),
            users: size,
            active,
            counts,
            percent,
        });
    }

    // --- weighted average row ----------------------------------------------
    let avg_active: Vec<Option<usize>> = (0..=periods)
        .map(|p| if sum_size[p] > 0 { Some(sum_active[p]) } else { None })
        .collect();
    let (mut acounts, mut apercent) = (Vec::new(), Vec::new());
    for p in 0..=periods {
        match metric {
            Metric::Retention => {
                acounts.push(avg_active[p].map(|a| a as i64));
                apercent.push(match (avg_active[p], sum_size[p]) {
                    (Some(a), s) if s > 0 => Some(pct(a as f64 / s as f64)),
                    _ => None,
                });
            }
            Metric::Churn => {
                if p == 0 {
                    acounts.push(avg_active[0].map(|_| 0));
                    apercent.push(avg_active[0].map(|_| 0.0));
                } else {
                    match (avg_active[p - 1], avg_active[p]) {
                        (Some(prev), Some(cur)) => {
                            acounts.push(Some(prev as i64 - cur as i64));
                            apercent.push(if prev > 0 {
                                Some(pct((prev as f64 - cur as f64) / prev as f64))
                            } else {
                                None
                            });
                        }
                        _ => {
                            acounts.push(None);
                            apercent.push(None);
                        }
                    }
                }
            }
        }
    }

    Ok(Report {
        metric: match metric {
            Metric::Retention => "retention".into(),
            Metric::Churn => "churn".into(),
        },
        granularity: match gran {
            Gran::Month => "month".into(),
            Gran::Week => "week".into(),
            Gran::Day => "day".into(),
        },
        periods,
        as_of: as_of.iso(),
        total_users,
        cohorts,
        average: AverageRow {
            users: total_users,
            counts: acounts,
            percent: apercent,
        },
        pre_signup_rows,
        unknown_users,
    })
}

/// Turn per-period active counts into the metric's counts + percentages.
fn derive(metric: Metric, active: &[Option<usize>], size: usize) -> (Vec<Option<i64>>, Vec<Option<f64>>) {
    let mut counts = Vec::with_capacity(active.len());
    let mut percent = Vec::with_capacity(active.len());
    for p in 0..active.len() {
        match metric {
            Metric::Retention => {
                counts.push(active[p].map(|a| a as i64));
                percent.push(active[p].map(|a| {
                    if size > 0 {
                        pct(a as f64 / size as f64)
                    } else {
                        0.0
                    }
                }));
            }
            Metric::Churn => {
                if p == 0 {
                    counts.push(active[0].map(|_| 0));
                    percent.push(active[0].map(|_| 0.0));
                } else {
                    match (active[p - 1], active[p]) {
                        (Some(prev), Some(cur)) => {
                            counts.push(Some(prev as i64 - cur as i64));
                            percent.push(if prev > 0 {
                                Some(pct((prev as f64 - cur as f64) / prev as f64))
                            } else {
                                None
                            });
                        }
                        _ => {
                            counts.push(None);
                            percent.push(None);
                        }
                    }
                }
            }
        }
    }
    (counts, percent)
}

// ---------------------------------------------------------------------------
// rendering
// ---------------------------------------------------------------------------

fn fmt_cell(values: Values, count: Option<i64>, percent: Option<f64>) -> String {
    match values {
        Values::Percent => match percent {
            Some(p) => format!("{p}%"),
            None => "-".into(),
        },
        Values::Count => match count {
            Some(c) => c.to_string(),
            None => "-".into(),
        },
        Values::Both => match (count, percent) {
            (Some(c), Some(p)) => format!("{c} ({p}%)"),
            (Some(c), None) => c.to_string(),
            _ => "-".into(),
        },
    }
}

fn pad(s: &str, w: usize) -> String {
    let mut out = s.to_string();
    while out.chars().count() < w {
        out.push(' ');
    }
    out
}

fn render_table(r: &Report, values: Values) -> String {
    let churn = r.metric == "churn";
    let gran_adj = match r.granularity.as_str() {
        "week" => "weekly",
        "day" => "daily",
        _ => "monthly",
    };
    let cells_note = match (values, churn) {
        (Values::Percent, _) => String::new(),
        (Values::Count, false) => ", cells = active users".into(),
        (Values::Count, true) => ", cells = users lost".into(),
        (Values::Both, false) => ", cells = active users (% of cohort)".into(),
        (Values::Both, true) => ", cells = users lost (% of previous period)".into(),
    };
    let mut out = format!(
        "Cohort {} ({gran_adj} cohorts): {} cohort(s), {} user(s), periods P0-P{}, as of {}{}\n",
        if churn { "churn" } else { "retention" },
        r.cohorts.len(),
        r.total_users,
        r.periods,
        r.as_of,
        cells_note
    );

    let mut header: Vec<String> = vec!["cohort".into(), "users".into()];
    for p in 0..=r.periods {
        header.push(format!("P{p}"));
    }
    let mut grid: Vec<Vec<String>> = vec![header];
    for c in &r.cohorts {
        let mut row = vec![c.cohort.clone(), c.users.to_string()];
        for p in 0..=r.periods {
            row.push(fmt_cell(values, c.counts[p], c.percent[p]));
        }
        grid.push(row);
    }
    let mut avg = vec!["average".to_string(), r.average.users.to_string()];
    for p in 0..=r.periods {
        avg.push(fmt_cell(values, r.average.counts[p], r.average.percent[p]));
    }
    grid.push(avg);

    let cols = grid[0].len();
    let widths: Vec<usize> = (0..cols)
        .map(|i| grid.iter().map(|row| row[i].chars().count()).max().unwrap_or(0))
        .collect();
    for row in &grid {
        let line: Vec<String> = row
            .iter()
            .enumerate()
            .map(|(i, c)| pad(c, widths[i]))
            .collect();
        out.push_str(line.join("  ").trim_end());
        out.push('\n');
    }

    // Curve of the weighted average.
    out.push('\n');
    out.push_str(&format!(
        "{} curve (weighted average of observable cohorts)\n",
        if churn { "Churn" } else { "Retention" }
    ));
    let lbl_w = format!("P{}", r.periods).chars().count();
    let val_w = (0..=r.periods)
        .map(|p| match r.average.percent[p] {
            Some(v) => format!("{v}%").chars().count(),
            None => 1,
        })
        .max()
        .unwrap_or(1);
    for p in 0..=r.periods {
        let v = r.average.percent[p];
        let txt = match v {
            Some(v) => format!("{v}%"),
            None => "-".into(),
        };
        let bars = match v {
            Some(v) => ((v.max(0.0) / 5.0).round() as usize).min(20),
            None => 0,
        };
        out.push_str(
            format!(
                "{}  {}  {}\n",
                pad(&format!("P{p}"), lbl_w),
                pad(&txt, val_w),
                "#".repeat(bars)
            )
            .trim_end(),
        );
        out.push('\n');
    }

    if r.pre_signup_rows > 0 {
        out.push_str(&format!(
            "\nNote: {} activity row(s) dated before the user's signup were ignored.\n",
            r.pre_signup_rows
        ));
    }
    if r.unknown_users > 0 {
        out.push_str(&format!(
            "\nNote: {} user(s) in the activity data are missing from the signup table and were ignored.\n",
            r.unknown_users
        ));
    }
    out
}

fn render_csv(r: &Report, values: Values) -> String {
    let mut out = String::from("cohort,users");
    for p in 0..=r.periods {
        out.push_str(&format!(",P{p}"));
    }
    out.push('\n');
    let cell_csv = |count: Option<i64>, percent: Option<f64>| -> String {
        match values {
            Values::Percent => percent.map(|p| p.to_string()).unwrap_or_default(),
            Values::Count => count.map(|c| c.to_string()).unwrap_or_default(),
            Values::Both => match (count, percent) {
                (Some(c), Some(p)) => format!("\"{c} ({p}%)\""),
                (Some(c), None) => c.to_string(),
                _ => String::new(),
            },
        }
    };
    for c in &r.cohorts {
        out.push_str(&format!("{},{}", c.cohort, c.users));
        for p in 0..=r.periods {
            out.push(',');
            out.push_str(&cell_csv(c.counts[p], c.percent[p]));
        }
        out.push('\n');
    }
    out.push_str(&format!("average,{}", r.average.users));
    for p in 0..=r.periods {
        out.push(',');
        out.push_str(&cell_csv(r.average.counts[p], r.average.percent[p]));
    }
    out.push('\n');
    out
}

/// Full pipeline: parse + compute + render.
#[allow(clippy::too_many_arguments)]
pub fn analyze(
    data: &str,
    signups: &str,
    user_spec: &str,
    date_spec: &str,
    signup_date_spec: &str,
    granularity: &str,
    periods: f64,
    metric: &str,
    values: &str,
    as_of: &str,
    has_header: bool,
    delimiter: &str,
    format: &str,
) -> Result<String, String> {
    let values = Values::parse(values)?;
    let report = compute(
        data,
        signups,
        user_spec,
        date_spec,
        signup_date_spec,
        granularity,
        periods,
        metric,
        as_of,
        has_header,
        delimiter,
    )?;
    match format.trim().to_ascii_lowercase().as_str() {
        "" | "table" => Ok(render_table(&report, values)),
        "csv" => Ok(render_csv(&report, values)),
        "json" => serde_json::to_string_pretty(&report)
            .map_err(|e| format!("could not serialize the report: {e}")),
        other => Err(format!(
            "format must be table, csv, or json, got '{other}'"
        )),
    }
}

// ---------------------------------------------------------------------------
// tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const EVENTS: &str = "user,date\n\
u1,2024-01-05\nu1,2024-02-10\nu1,2024-03-02\n\
u2,2024-01-20\nu2,2024-02-15\n\
u3,2024-01-25\n\
u4,2024-02-03\nu4,2024-03-09\n\
u5,2024-03-11\n";

    #[test]
    fn monthly_retention_table_from_events_only() {
        let out = analyze(
            EVENTS, "", "user", "date", "", "month", 2.0, "retention", "percent", "", true, "comma",
            "table",
        )
        .unwrap();
        assert!(
            out.starts_with(
                "Cohort retention (monthly cohorts): 3 cohort(s), 5 user(s), periods P0-P2, as of 2024-03-11\n"
            ),
            "{out}"
        );
        // 2024-01 cohort: u1,u2,u3 -> P0 3/3, P1 2/3 (u1,u2), P2 1/3 (u1)
        assert!(out.contains("2024-01  3      100%  66.67%  33.33%"), "{out}");
        // 2024-02 cohort: u4 -> P0 100%, P1 100%; P2 unobservable
        assert!(out.contains("2024-02  1      100%  100%    -"), "{out}");
        // 2024-03 cohort: u5 -> only P0 observable
        assert!(out.contains("2024-03  1      100%  -       -"), "{out}");
        // weighted average: P1 = (2+1)/(3+1) = 75%, P2 = 1/3
        assert!(out.contains("average  5      100%  75%     33.33%"), "{out}");
        assert!(out.contains("Retention curve (weighted average of observable cohorts)"), "{out}");
        assert!(out.contains("P0  100%    ####################"), "{out}");
    }

    #[test]
    fn churn_metric_is_period_over_period() {
        let out = analyze(
            EVENTS, "", "user", "date", "", "month", 2.0, "churn", "percent", "", true, "comma",
            "table",
        )
        .unwrap();
        assert!(out.starts_with("Cohort churn (monthly cohorts):"), "{out}");
        // 2024-01: 3 -> 2 -> 1 => P1 33.33% churn, P2 50% churn
        assert!(out.contains("2024-01  3      0%  33.33%  50%"), "{out}");
        assert!(out.contains("Churn curve (weighted average of observable cohorts)"), "{out}");
    }

    #[test]
    fn signup_table_sets_the_cohort_and_p0_can_be_below_100() {
        let signups = "user,signup\nu1,2024-01-01\nu2,2024-01-02\nu3,2024-01-03\nu4,2024-01-04\n";
        let events = "user,date\nu1,2024-01-05\nu2,2024-02-15\nu3,2024-03-02\n";
        let out = analyze(
            events, signups, "user", "date", "signup", "month", 2.0, "retention", "both", "",
            true, "comma", "table",
        )
        .unwrap();
        // u4 never acted: P0 = 1/4 (only u1 active in January).
        assert!(out.contains("2024-01  4      1 (25%)  1 (25%)  1 (25%)"), "{out}");
        assert!(out.contains("cells = active users (% of cohort)"), "{out}");
    }

    #[test]
    fn users_missing_from_the_signup_table_are_reported() {
        let signups = "user,signup\nu1,2024-01-01\n";
        let events = "user,date\nu1,2024-01-05\nu9,2024-01-06\n";
        let out = analyze(
            events, signups, "user", "date", "signup", "month", 1.0, "retention", "percent", "",
            true, "comma", "table",
        )
        .unwrap();
        assert!(
            out.contains("Note: 1 user(s) in the activity data are missing from the signup table"),
            "{out}"
        );
    }

    #[test]
    fn activity_before_signup_is_ignored_and_reported() {
        let signups = "user,signup\nu1,2024-02-01\n";
        let events = "user,date\nu1,2024-01-05\nu1,2024-02-07\n";
        let out = analyze(
            events, signups, "user", "date", "signup", "month", 1.0, "retention", "percent", "",
            true, "comma", "table",
        )
        .unwrap();
        assert!(
            out.contains("Note: 1 activity row(s) dated before the user's signup were ignored."),
            "{out}"
        );
    }

    #[test]
    fn weekly_cohorts_start_on_monday() {
        // 2024-01-04 is a Thursday; its week starts Monday 2024-01-01.
        let events = "user,date\nu1,2024-01-04\nu1,2024-01-09\n";
        let out = analyze(
            events, "", "user", "date", "", "week", 1.0, "retention", "percent", "", true, "comma",
            "table",
        )
        .unwrap();
        assert!(out.contains("2024-01-01  1      100%  100%"), "{out}");
    }

    #[test]
    fn daily_cohorts_and_as_of_extends_the_observable_window() {
        let events = "user,date\nu1,2024-01-01\nu1,2024-01-02\n";
        let out = analyze(
            events, "", "user", "date", "", "day", 3.0, "retention", "count", "2024-01-04", true,
            "comma", "table",
        )
        .unwrap();
        assert!(out.contains("as of 2024-01-04"), "{out}");
        // P3 is observable (2024-01-04) and empty -> 0, not "-".
        assert!(out.contains("2024-01-01  1      1   1   0   0"), "{out}");
    }

    #[test]
    fn csv_output_leaves_unobservable_cells_empty() {
        let out = analyze(
            EVENTS, "", "user", "date", "", "month", 2.0, "retention", "percent", "", true, "comma",
            "csv",
        )
        .unwrap();
        assert_eq!(
            out,
            "cohort,users,P0,P1,P2\n\
2024-01,3,100,66.67,33.33\n\
2024-02,1,100,100,\n\
2024-03,1,100,,\n\
average,5,100,75,33.33\n"
        );
    }

    #[test]
    fn json_output_is_structured_with_nulls() {
        let out = analyze(
            EVENTS, "", "user", "date", "", "month", 2.0, "retention", "percent", "", true, "comma",
            "json",
        )
        .unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["metric"], "retention");
        assert_eq!(v["granularity"], "month");
        assert_eq!(v["total_users"], 5);
        assert_eq!(v["as_of"], "2024-03-11");
        assert_eq!(v["cohorts"][0]["cohort"], "2024-01");
        assert_eq!(v["cohorts"][0]["percent"][1], 66.67);
        assert!(v["cohorts"][2]["percent"][1].is_null());
        assert_eq!(v["average"]["percent"][1], 75.0);
    }

    #[test]
    fn tab_delimiter_and_index_columns_without_a_header() {
        let events = "u1\t2024-01-05\nu1\t2024-02-05\nu2\t2024-01-06\n";
        let out = analyze(
            events, "", "1", "2", "", "month", 1.0, "retention", "percent", "", false, "tab",
            "table",
        )
        .unwrap();
        assert!(out.contains("2024-01  2      100%  50%"), "{out}");
    }

    #[test]
    fn epoch_and_iso_timestamps_parse() {
        assert_eq!(
            parse_date("1704067200", "d").unwrap(),
            Date { y: 2024, m: 1, d: 1 }
        );
        assert_eq!(
            parse_date("1704067200000", "d").unwrap(),
            Date { y: 2024, m: 1, d: 1 }
        );
        assert_eq!(
            parse_date("2024-01-15T09:30:00Z", "d").unwrap(),
            Date { y: 2024, m: 1, d: 15 }
        );
        assert_eq!(
            parse_date("2024-01-15 09:30:00", "d").unwrap(),
            Date { y: 2024, m: 1, d: 15 }
        );
        assert_eq!(parse_date("20240115", "d").unwrap(), Date { y: 2024, m: 1, d: 15 });
        assert_eq!(parse_date("2024/01/15", "d").unwrap(), Date { y: 2024, m: 1, d: 15 });
        assert_eq!(parse_date("2024-02", "d").unwrap(), Date { y: 2024, m: 2, d: 1 });
    }

    #[test]
    fn ambiguous_dates_are_rejected_not_guessed() {
        let e = parse_date("03/04/2024", "activity date").unwrap_err();
        assert!(e.contains("ambiguous"), "{e}");
        assert!(e.contains("YYYY-MM-DD"), "{e}");
    }

    #[test]
    fn invalid_calendar_dates_are_rejected() {
        let e = parse_date("2023-02-29", "d").unwrap_err();
        assert!(e.contains("28 day(s)"), "{e}");
        assert!(parse_date("2024-02-29", "d").is_ok());
        let e = parse_date("2024-13-01", "d").unwrap_err();
        assert!(e.contains("month 13"), "{e}");
    }

    #[test]
    fn empty_input_errors() {
        let e = analyze(
            "   ", "", "", "", "", "month", 6.0, "retention", "percent", "", true, "comma", "table",
        )
        .unwrap_err();
        assert!(e.contains("activity data is empty"), "{e}");
    }

    #[test]
    fn unknown_column_errors_name_the_column() {
        let e = analyze(
            EVENTS, "", "customer", "date", "", "month", 6.0, "retention", "percent", "", true,
            "comma", "table",
        )
        .unwrap_err();
        assert_eq!(e, "user column 'customer' not found in the header row");
    }

    #[test]
    fn period_bounds_are_enforced() {
        let too_low = analyze(
            EVENTS, "", "", "", "", "month", 0.0, "retention", "percent", "", true, "comma", "table",
        )
        .unwrap_err();
        assert!(too_low.contains("between 1 and 36"), "{too_low}");
        let too_high = analyze(
            EVENTS, "", "", "", "", "month", 37.0, "retention", "percent", "", true, "comma",
            "table",
        )
        .unwrap_err();
        assert!(too_high.contains("between 1 and 36"), "{too_high}");
        // The boundary values themselves are fine.
        assert!(analyze(
            EVENTS, "", "", "", "", "month", 1.0, "retention", "percent", "", true, "comma", "table"
        )
        .is_ok());
        assert!(analyze(
            EVENTS, "", "", "", "", "month", 36.0, "retention", "percent", "", true, "comma",
            "table"
        )
        .is_ok());
    }

    #[test]
    fn bad_enums_error_clearly() {
        for (g, m, v, f, needle) in [
            ("year", "retention", "percent", "table", "granularity must be"),
            ("month", "growth", "percent", "table", "metric must be"),
            ("month", "retention", "ratio", "table", "values must be"),
            ("month", "retention", "percent", "xlsx", "format must be"),
        ] {
            let e = analyze(EVENTS, "", "", "", "", g, 2.0, m, v, "", true, "comma", f).unwrap_err();
            assert!(e.contains(needle), "{e}");
        }
    }

    #[test]
    fn too_many_cohorts_is_capped() {
        let mut data = String::from("user,date\n");
        for i in 0..(MAX_COHORTS + 1) {
            let d = civil_from_days(19_000 + i as i64);
            data.push_str(&format!("u{i},{}\n", d.iso()));
        }
        let e = analyze(
            &data, "", "user", "date", "", "day", 1.0, "retention", "percent", "", true, "comma",
            "table",
        )
        .unwrap_err();
        assert!(e.contains("max 1000"), "{e}");
        assert!(e.contains("coarser granularity"), "{e}");
    }

    #[test]
    fn row_cap_is_enforced() {
        let mut data = String::from("user,date\n");
        for i in 0..=MAX_ROWS {
            data.push_str(&format!("u{i},2024-01-01\n"));
        }
        let e = analyze(
            &data, "", "user", "date", "", "month", 1.0, "retention", "percent", "", true, "comma",
            "table",
        )
        .unwrap_err();
        assert!(e.contains("more than 50000 rows"), "{e}");
    }

    #[test]
    fn civil_date_roundtrip() {
        for z in [-1_000_i64, 0, 1, 19_000, 25_000] {
            let d = civil_from_days(z);
            assert_eq!(d.epoch_days(), z, "{d:?}");
        }
    }
}
