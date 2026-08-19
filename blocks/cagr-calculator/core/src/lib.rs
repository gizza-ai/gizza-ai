//! cagr-calculator core — compound annual growth rate and period-over-period
//! growth for a series of VALUES (levels: revenue, GDP, price, headcount), not
//! a series of returns. Pure compute, shared by the chat skill block and the
//! web page. No wafer/wasm-bindgen deps.
//!
//! CAGR is the constant rate that takes the first value to the last one over
//! the elapsed time: `(end / start)^(1 / years) - 1`. The elapsed time comes
//! from the series spacing (`period`), an explicit `years` override, or an
//! exact `start_date`/`end_date` pair — in that order of precedence.
//!
//! Educational only — not financial advice.

use serde::Serialize;

/// Largest accepted series length. Well past any pasted spreadsheet column and
/// small enough that the rendered table stays usable.
pub const MAX_POINTS: usize = 2000;

/// One parsed data point: an optional row label (a year, a quarter, a month
/// name) and its numeric value.
#[derive(Debug, Clone, Serialize)]
pub struct Point {
    /// The row label as typed (`"2000"`, `"Q1 2024"`), or the 1-based position
    /// when the series had no label column.
    pub label: String,
    pub value: f64,
}

/// One period-over-period step between two consecutive points.
#[derive(Debug, Clone, Serialize)]
pub struct Step {
    pub from_label: String,
    pub to_label: String,
    pub from: f64,
    pub to: f64,
    /// Absolute change over the step.
    pub change: f64,
    /// Growth over the step as a decimal (0.12 = +12%). `None` when the
    /// starting value of the step is 0 and the percentage is undefined.
    pub pct_change: Option<f64>,
    /// Value rebased so the first point of the whole series equals 100.
    /// `None` when the first value is 0.
    pub index: Option<f64>,
}

/// The full analysis of a value series.
#[derive(Debug, Clone, Serialize)]
pub struct Analysis {
    /// Number of data points parsed.
    pub count: usize,
    /// Number of steps between points (`count - 1`).
    pub periods: usize,
    pub start_label: String,
    pub end_label: String,
    pub start_value: f64,
    pub end_value: f64,
    /// Elapsed time in years used as the CAGR exponent.
    pub elapsed_years: f64,
    /// Where `elapsed_years` came from: `"period"`, `"years"` or `"dates"`.
    pub elapsed_years_source: String,
    /// Compound annual growth rate as a decimal (0.0845 = 8.45% / yr).
    pub cagr: f64,
    /// Compound growth per SERIES period as a decimal — the same rate expressed
    /// per step rather than per year.
    pub compound_period_growth: f64,
    /// Total growth from first to last value as a decimal (0.5 = +50%).
    pub total_growth: f64,
    /// Last value ÷ first value.
    pub multiple: f64,
    /// Last value − first value.
    pub absolute_change: f64,
    /// Arithmetic mean of the period-over-period growth rates (decimal).
    /// Always ≥ CAGR for a volatile series — see `note`.
    pub mean_period_growth: Option<f64>,
    /// Best single period: (label of the step's end point, growth decimal).
    pub best_period: Option<(String, f64)>,
    /// Worst single period.
    pub worst_period: Option<(String, f64)>,
    /// Years for the value to double at the computed CAGR. `None` unless the
    /// CAGR is positive.
    pub doubling_years: Option<f64>,
    /// The `target_value` echoed back, when one was supplied.
    pub target_value: Option<f64>,
    /// Years from the LAST value to reach `target_value` at the computed CAGR.
    /// Negative means the series already passed the target. `None` when the
    /// CAGR is not positive or the target is not positive.
    pub years_to_target: Option<f64>,
    pub steps: Vec<Step>,
    pub note: String,
}

/// Series spacing → periods per year. Daily uses 365 CALENDAR days; a
/// 252-trading-day price series should pass an explicit `years` override.
fn periods_per_year(period: &str) -> Result<f64, String> {
    match period.trim().to_ascii_lowercase().as_str() {
        "annual" | "" => Ok(1.0),
        "quarterly" => Ok(4.0),
        "monthly" => Ok(12.0),
        "weekly" => Ok(52.0),
        "daily" => Ok(365.0),
        other => Err(format!(
            "period must be one of annual, quarterly, monthly, weekly, daily — got '{other}'"
        )),
    }
}

fn round6(x: f64) -> f64 {
    (x * 1e6).round() / 1e6
}

/// Is every comma in `s` a valid thousands separator (a 1–3 digit lead group,
/// then groups of exactly 3 digits)? This is what keeps `1,234` a single number
/// while `2000,100` stays a `label,value` row — a real thousands-grouped number
/// never has 4+ digits before its first comma.
fn commas_are_thousands(s: &str) -> bool {
    let b: Vec<char> = s.chars().collect();
    let first = match b.iter().position(|c| *c == ',') {
        Some(i) => i,
        None => return false,
    };
    // The lead group: 1–3 digits, optionally preceded by a sign.
    let lead: Vec<char> = b[..first].iter().copied().collect();
    let digits_start = usize::from(matches!(lead.first(), Some('+' | '-')));
    let lead_digits = &lead[digits_start..];
    if lead_digits.is_empty()
        || lead_digits.len() > 3
        || !lead_digits.iter().all(|c| c.is_ascii_digit())
    {
        return false;
    }
    for (i, c) in b.iter().enumerate() {
        if *c != ',' {
            continue;
        }
        // Exactly three digits must follow, then end / `.` / the next group.
        let after: Vec<char> = b[i + 1..].iter().take(4).copied().collect();
        if after.len() < 3 || !after[..3].iter().all(|c| c.is_ascii_digit()) {
            return false;
        }
        if after.len() == 4 && after[3] != '.' && after[3] != ',' {
            return false;
        }
    }
    true
}

/// Parse one numeric cell, tolerating what people actually paste out of a
/// spreadsheet: currency symbols, thousands separators (comma, space,
/// underscore), a trailing/leading sign, and accounting parentheses for
/// negatives.
pub fn parse_number(raw: &str) -> Option<f64> {
    let mut t = raw.trim();
    if t.is_empty() {
        return None;
    }
    // Accounting negatives: (1,234) = -1234.
    let mut negate = false;
    if t.starts_with('(') && t.ends_with(')') && t.len() >= 3 {
        negate = true;
        t = t[1..t.len() - 1].trim();
    }
    let mut s: String = t
        .chars()
        .filter(|c| {
            !matches!(c, '$' | '€' | '£' | '¥' | '₹' | '₽' | '"' | '\'' | '\u{a0}' | '\u{202f}')
        })
        .collect();
    s = s.trim().to_string();
    if s.is_empty() {
        return None;
    }
    if s.contains(',') {
        if !commas_are_thousands(&s) {
            return None;
        }
        s = s.replace(',', "");
    }
    s = s.replace(['_', ' '], "");
    // A leading `+` is fine for f64, a trailing one is not.
    let v: f64 = s.parse().ok()?;
    if !v.is_finite() {
        return None;
    }
    Some(if negate { -v } else { v })
}

/// Split a labelled row into `(label, value)`. `label,1,234` keeps the
/// thousands-grouped value by re-joining the tail before falling back to the
/// last field (so `2000,United States,1234` still works).
fn split_labelled(line: &str, sep: char) -> Option<(String, f64)> {
    let fields: Vec<&str> = line.split(sep).collect();
    if fields.len() < 2 {
        return None;
    }
    let label = fields[0].trim().trim_matches('"').to_string();
    let joined = fields[1..].join(&sep.to_string());
    if let Some(v) = parse_number(&joined) {
        return Some((label, v));
    }
    let last = fields[fields.len() - 1];
    parse_number(last).map(|v| (label, v))
}

/// Parse the pasted series into points. One point per line; a lone line is
/// instead split into many values on commas/spaces (so `100, 150, 210` works).
pub fn parse_series(raw: &str, has_header: bool) -> Result<Vec<Point>, String> {
    let mut lines: Vec<&str> = raw
        .lines()
        .map(|l| l.trim_end_matches('\r').trim())
        .filter(|l| !l.is_empty())
        .collect();
    if has_header && !lines.is_empty() {
        lines.remove(0);
    }
    if lines.is_empty() {
        return Err("no values found — paste at least 2 numbers, one per line or separated by commas".into());
    }

    // Single line → a delimiter-separated list of values.
    if lines.len() == 1 {
        let parts: Vec<&str> = lines[0]
            .split([',', ';', '\t', ' '])
            .map(|p| p.trim())
            .filter(|p| !p.is_empty())
            .collect();
        let mut pts = Vec::new();
        for (i, p) in parts.iter().enumerate() {
            let v = parse_number(p).ok_or_else(|| {
                format!("value '{p}' is not a number (a one-line series must be all numbers, e.g. 100, 150, 210)")
            })?;
            pts.push(Point { label: (i + 1).to_string(), value: v });
        }
        return finish_series(pts);
    }

    let mut pts = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if let Some(v) = parse_number(line) {
            pts.push(Point { label: (i + 1).to_string(), value: v });
            continue;
        }
        let parsed = ['\t', ';', ','].iter().find_map(|s| split_labelled(line, *s));
        match parsed {
            Some((label, value)) => {
                let label = if label.is_empty() { (i + 1).to_string() } else { label };
                pts.push(Point { label, value });
            }
            None => {
                return Err(format!(
                    "line {} ('{}') is not a number or a label,value pair — write one value per line, optionally as '2000,1234'",
                    i + 1,
                    line
                ))
            }
        }
    }
    finish_series(pts)
}

fn finish_series(pts: Vec<Point>) -> Result<Vec<Point>, String> {
    if pts.len() < 2 {
        return Err(format!(
            "need at least 2 values to measure growth, got {} — add the ending value",
            pts.len()
        ));
    }
    if pts.len() > MAX_POINTS {
        return Err(format!(
            "series has {} values, the maximum is {MAX_POINTS}",
            pts.len()
        ));
    }
    Ok(pts)
}

/// Days since 1970-01-01 for a proleptic-Gregorian y/m/d (Howard Hinnant's
/// `days_from_civil`). Avoids pulling a date crate into the page wasm.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Parse `YYYY-MM-DD` (also accepts `/` separators) into days since epoch.
fn parse_date(raw: &str, field: &str) -> Result<i64, String> {
    let t = raw.trim();
    let parts: Vec<&str> = t.split(['-', '/']).collect();
    let bad = || format!("{field} must be a date like 2000-01-31, got '{t}'");
    if parts.len() != 3 {
        return Err(bad());
    }
    let y: i64 = parts[0].parse().map_err(|_| bad())?;
    let m: i64 = parts[1].parse().map_err(|_| bad())?;
    let d: i64 = parts[2].parse().map_err(|_| bad())?;
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return Err(bad());
    }
    Ok(days_from_civil(y, m, d))
}

/// Analyse a value series.
///
/// * `values` — the pasted series (see [`parse_series`]).
/// * `period` — spacing between consecutive values: annual/quarterly/monthly/weekly/daily.
/// * `years` — explicit elapsed years; `0` (or negative) means "derive it".
/// * `start_date`/`end_date` — an exact date range, used when `years` is unset.
/// * `target_value` — optional goal for the years-to-target projection; `0` = none.
/// * `has_header` — drop the first line before parsing.
#[allow(clippy::too_many_arguments)]
pub fn analyze(
    values: &str,
    period: &str,
    years: f64,
    start_date: &str,
    end_date: &str,
    target_value: f64,
    has_header: bool,
) -> Result<Analysis, String> {
    let pts = parse_series(values, has_header)?;
    let ppy = periods_per_year(period)?;
    let periods = pts.len() - 1;

    let (elapsed_years, source) = if years > 0.0 {
        if !years.is_finite() {
            return Err("years must be a finite number".into());
        }
        (years, "years")
    } else if !start_date.trim().is_empty() || !end_date.trim().is_empty() {
        if start_date.trim().is_empty() || end_date.trim().is_empty() {
            return Err("give BOTH start_date and end_date to use an exact date range (or leave both blank)".into());
        }
        let a = parse_date(start_date, "start_date")?;
        let b = parse_date(end_date, "end_date")?;
        if b <= a {
            return Err("end_date must be after start_date".into());
        }
        ((b - a) as f64 / 365.25, "dates")
    } else {
        (periods as f64 / ppy, "period")
    };
    if elapsed_years <= 0.0 {
        return Err("elapsed time must be greater than 0 years".into());
    }

    let start_value = pts[0].value;
    let end_value = pts[pts.len() - 1].value;
    if start_value <= 0.0 {
        return Err(format!(
            "CAGR needs a positive starting value, got {start_value} — a growth rate from zero or a negative base is undefined"
        ));
    }
    if end_value < 0.0 {
        return Err(format!(
            "CAGR is undefined for a negative ending value ({end_value}) — the series would have to cross zero"
        ));
    }

    let multiple = end_value / start_value;
    let cagr = multiple.powf(1.0 / elapsed_years) - 1.0;
    let compound_period_growth = multiple.powf(1.0 / periods as f64) - 1.0;
    let total_growth = multiple - 1.0;

    let mut steps = Vec::with_capacity(periods);
    let mut growths: Vec<(String, f64)> = Vec::with_capacity(periods);
    for w in pts.windows(2) {
        let (a, b) = (&w[0], &w[1]);
        let pct = if a.value == 0.0 { None } else { Some(round6(b.value / a.value - 1.0)) };
        if let Some(p) = pct {
            growths.push((b.label.clone(), p));
        }
        steps.push(Step {
            from_label: a.label.clone(),
            to_label: b.label.clone(),
            from: a.value,
            to: b.value,
            change: round6(b.value - a.value),
            pct_change: pct,
            index: if start_value == 0.0 { None } else { Some(round6(b.value / start_value * 100.0)) },
        });
    }

    let mean_period_growth = if growths.is_empty() {
        None
    } else {
        Some(round6(growths.iter().map(|g| g.1).sum::<f64>() / growths.len() as f64))
    };
    let best_period = growths
        .iter()
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        .cloned();
    let worst_period = growths
        .iter()
        .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        .cloned();

    let doubling_years = if cagr > 0.0 {
        Some(round6(2f64.ln() / (1.0 + cagr).ln()))
    } else {
        None
    };
    let (target, years_to_target) = if target_value > 0.0 {
        let yrs = if cagr > 0.0 && end_value > 0.0 {
            Some(round6((target_value / end_value).ln() / (1.0 + cagr).ln()))
        } else {
            None
        };
        (Some(target_value), yrs)
    } else {
        (None, None)
    };

    let note = format!(
        "CAGR is the constant annual rate that turns the first value into the last one over {} — it smooths away everything in between, so it is not the average of the period growth rates and says nothing about volatility. Educational only, not financial advice.",
        match source {
            "years" => "the years you supplied".to_string(),
            "dates" => "the exact date range".to_string(),
            _ => {
                let p = period.trim().to_ascii_lowercase();
                let p = if p.is_empty() { "annual".to_string() } else { p };
                plural(periods, &format!("{p} period"))
            }
        }
    );

    Ok(Analysis {
        count: pts.len(),
        periods,
        start_label: pts[0].label.clone(),
        end_label: pts[pts.len() - 1].label.clone(),
        start_value,
        end_value,
        elapsed_years: round6(elapsed_years),
        elapsed_years_source: source.to_string(),
        cagr: round6(cagr),
        compound_period_growth: round6(compound_period_growth),
        total_growth: round6(total_growth),
        multiple: round6(multiple),
        absolute_change: round6(end_value - start_value),
        mean_period_growth,
        best_period,
        worst_period,
        doubling_years,
        target_value: target,
        years_to_target,
        steps,
        note,
    })
}

// ---------------------------------------------------------------- formatting

fn group_thousands(int_part: &str) -> String {
    let neg = int_part.starts_with('-');
    let digits = if neg { &int_part[1..] } else { int_part };
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    if neg {
        format!("-{out}")
    } else {
        out
    }
}

/// Format a value with thousands separators, trimming trailing zeros (so
/// `1234.0` prints `1,234` and `1.055` prints `1.055`).
pub fn fmt_value(v: f64) -> String {
    let s = format!("{v:.6}");
    let s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    match s.split_once('.') {
        Some((i, f)) => format!("{}.{}", group_thousands(i), f),
        None => group_thousands(&s),
    }
}

/// Format a decimal rate as a signed percentage with `dp` decimals.
pub fn fmt_pct(v: f64, dp: usize) -> String {
    format!("{:+.*}%", dp, v * 100.0)
}

/// Format a duration in years for display — 2 decimals is the useful precision
/// for "8.55 years to double"; whole numbers stay whole.
fn fmt_years(v: f64) -> String {
    fmt_value((v * 100.0).round() / 100.0)
}

/// `1 period` / `3 periods`.
fn plural(n: usize, word: &str) -> String {
    if n == 1 {
        format!("{n} {word}")
    } else {
        format!("{n} {word}s")
    }
}

fn pad(s: &str, w: usize) -> String {
    let n = s.chars().count();
    if n >= w {
        s.to_string()
    } else {
        format!("{s}{}", " ".repeat(w - n))
    }
}

fn lpad(s: &str, w: usize) -> String {
    let n = s.chars().count();
    if n >= w {
        s.to_string()
    } else {
        format!("{}{s}", " ".repeat(w - n))
    }
}

fn table_rows(a: &Analysis, dp: usize) -> Vec<[String; 5]> {
    let mut rows = vec![[
        "period".to_string(),
        "value".to_string(),
        "change".to_string(),
        "growth".to_string(),
        "index".to_string(),
    ]];
    rows.push([
        a.start_label.clone(),
        fmt_value(a.start_value),
        "—".to_string(),
        "—".to_string(),
        if a.start_value == 0.0 { "—".to_string() } else { "100.00".to_string() },
    ]);
    for s in &a.steps {
        rows.push([
            s.to_label.clone(),
            fmt_value(s.to),
            fmt_value(s.change),
            s.pct_change.map(|p| fmt_pct(p, dp)).unwrap_or_else(|| "—".to_string()),
            s.index.map(|i| format!("{i:.2}")).unwrap_or_else(|| "—".to_string()),
        ]);
    }
    rows
}

fn render_table(a: &Analysis, dp: usize) -> String {
    let rows = table_rows(a, dp);
    let mut widths = [0usize; 5];
    for r in &rows {
        for (i, c) in r.iter().enumerate() {
            widths[i] = widths[i].max(c.chars().count());
        }
    }
    let mut out = String::new();
    for (n, r) in rows.iter().enumerate() {
        let line = format!(
            "{}  {}  {}  {}  {}",
            pad(&r[0], widths[0]),
            lpad(&r[1], widths[1]),
            lpad(&r[2], widths[2]),
            lpad(&r[3], widths[3]),
            lpad(&r[4], widths[4]),
        );
        out.push_str(line.trim_end());
        out.push('\n');
        if n == 0 {
            let total: usize = widths.iter().sum::<usize>() + 8;
            out.push_str(&"-".repeat(total));
            out.push('\n');
        }
    }
    out.trim_end().to_string()
}

fn render_csv(a: &Analysis, dp: usize) -> String {
    let mut out = String::from("period,value,change,growth_pct,index\n");
    out.push_str(&format!(
        "{},{},,,{}\n",
        csv_cell(&a.start_label),
        a.start_value,
        if a.start_value == 0.0 { String::new() } else { "100".to_string() }
    ));
    for s in &a.steps {
        out.push_str(&format!(
            "{},{},{},{},{}\n",
            csv_cell(&s.to_label),
            s.to,
            s.change,
            s.pct_change.map(|p| format!("{:.*}", dp, p * 100.0)).unwrap_or_default(),
            s.index.map(|i| format!("{i:.2}")).unwrap_or_default(),
        ));
    }
    out.trim_end().to_string()
}

fn csv_cell(s: &str) -> String {
    if s.contains([',', '"', '\n']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn render_summary(a: &Analysis, dp: usize) -> String {
    let mut out = String::new();
    out.push_str(&format!("CAGR: {} per year\n\n", fmt_pct(a.cagr, dp)));
    out.push_str(&format!(
        "start ({}): {}\nend ({}): {}\n",
        a.start_label,
        fmt_value(a.start_value),
        a.end_label,
        fmt_value(a.end_value)
    ));
    out.push_str(&format!(
        "elapsed: {} years ({}, {}, from {})\n",
        fmt_years(a.elapsed_years),
        plural(a.count, "data point"),
        plural(a.periods, "period"),
        match a.elapsed_years_source.as_str() {
            "years" => "the years you set",
            "dates" => "your date range",
            _ => "the series spacing",
        }
    ));
    out.push_str(&format!(
        "total growth: {}  ({}x, {} absolute)\n",
        fmt_pct(a.total_growth, dp),
        fmt_value(a.multiple),
        fmt_value(a.absolute_change)
    ));
    out.push_str(&format!(
        "compound growth per period: {}\n",
        fmt_pct(a.compound_period_growth, dp)
    ));
    if let Some(m) = a.mean_period_growth {
        out.push_str(&format!("average period growth (arithmetic): {}\n", fmt_pct(m, dp)));
    }
    if let Some((l, v)) = &a.best_period {
        out.push_str(&format!("best period: {} at {}\n", fmt_pct(*v, dp), l));
    }
    if let Some((l, v)) = &a.worst_period {
        out.push_str(&format!("worst period: {} at {}\n", fmt_pct(*v, dp), l));
    }
    match a.doubling_years {
        Some(d) => out.push_str(&format!("doubling time at this CAGR: {} years\n", fmt_years(d))),
        None => out.push_str("doubling time: n/a (growth is not positive)\n"),
    }
    if let Some(t) = a.target_value {
        match a.years_to_target {
            Some(y) if y >= 0.0 => out.push_str(&format!(
                "reaching {} from the last value: {} more years at this CAGR\n",
                fmt_value(t),
                fmt_years(y)
            )),
            Some(y) => out.push_str(&format!(
                "target {} is already behind you (passed about {} years ago at this CAGR)\n",
                fmt_value(t),
                fmt_years(-y)
            )),
            None => out.push_str(&format!(
                "target {}: unreachable — growth is not positive\n",
                fmt_value(t)
            )),
        }
    }
    out.push('\n');
    out.push_str(&render_table(a, dp));
    out.push_str("\n\n");
    out.push_str(&a.note);
    out
}

/// Analyse a series and render it in the requested `output` shape:
/// `summary` (metrics + table), `table`, `csv`, or `json`.
#[allow(clippy::too_many_arguments)]
pub fn summary(
    values: &str,
    period: &str,
    years: f64,
    start_date: &str,
    end_date: &str,
    target_value: f64,
    has_header: bool,
    decimals: i64,
    output: &str,
) -> Result<String, String> {
    if !(0..=10).contains(&decimals) {
        return Err(format!("decimals must be between 0 and 10, got {decimals}"));
    }
    let dp = decimals as usize;
    let a = analyze(values, period, years, start_date, end_date, target_value, has_header)?;
    match output.trim().to_ascii_lowercase().as_str() {
        "summary" | "" => Ok(render_summary(&a, dp)),
        "table" => Ok(render_table(&a, dp)),
        "csv" => Ok(render_csv(&a, dp)),
        "json" => serde_json::to_string_pretty(&a).map_err(|e| e.to_string()),
        other => Err(format!(
            "output must be one of summary, table, csv, json — got '{other}'"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-6, "{a} != {b}");
    }

    #[test]
    fn classic_two_value_cagr() {
        // 100,000 → 150,000 over 5 years = 8.4472% / yr.
        let a = analyze("100000\n150000", "annual", 5.0, "", "", 0.0, false).unwrap();
        approx(a.cagr, 0.084472);
        approx(a.total_growth, 0.5);
        approx(a.elapsed_years, 5.0);
        assert_eq!(a.elapsed_years_source, "years");
        assert_eq!(a.periods, 1);
    }

    #[test]
    fn annual_series_derives_years_from_spacing() {
        // 4 annual points = 3 years elapsed. 100 → 133.1 = exactly 10% / yr.
        let a = analyze("100\n110\n121\n133.1", "annual", 0.0, "", "", 0.0, false).unwrap();
        approx(a.elapsed_years, 3.0);
        approx(a.cagr, 0.1);
        approx(a.compound_period_growth, 0.1);
        assert_eq!(a.elapsed_years_source, "period");
        assert_eq!(a.steps.len(), 3);
        approx(a.steps[0].pct_change.unwrap(), 0.1);
        approx(a.steps[2].index.unwrap(), 133.1);
    }

    #[test]
    fn monthly_period_annualizes() {
        // 13 monthly points = 12 months = 1 year; 100 → 200 = +100% / yr.
        let series: Vec<String> = (0..13).map(|i| (100.0 * 2f64.powf(i as f64 / 12.0)).to_string()).collect();
        let a = analyze(&series.join("\n"), "monthly", 0.0, "", "", 0.0, false).unwrap();
        approx(a.elapsed_years, 1.0);
        approx(a.cagr, 1.0);
    }

    #[test]
    fn labelled_rows_and_thousands_separators() {
        let a = analyze("2000,1,234\n2001,\"2,468\"\n2002,4936", "annual", 0.0, "", "", 0.0, false).unwrap();
        assert_eq!(a.start_label, "2000");
        assert_eq!(a.end_label, "2002");
        approx(a.start_value, 1234.0);
        approx(a.end_value, 4936.0);
        approx(a.cagr, 1.0); // doubling each year
    }

    #[test]
    fn one_line_series_and_currency_cells() {
        let a = analyze("$100, $150, $210", "annual", 0.0, "", "", 0.0, false).unwrap();
        assert_eq!(a.count, 3);
        approx(a.start_value, 100.0);
        approx(a.end_value, 210.0);
    }

    #[test]
    fn header_row_is_skipped() {
        let a = analyze("year,gdp\n2000,100\n2001,150", "annual", 0.0, "", "", 0.0, true).unwrap();
        assert_eq!(a.count, 2);
        assert_eq!(a.start_label, "2000");
        approx(a.total_growth, 0.5);
    }

    #[test]
    fn accounting_negative_values_parse() {
        assert_eq!(parse_number("(1,234)"), Some(-1234.0));
        assert_eq!(parse_number("1 234.5"), Some(1234.5));
        assert_eq!(parse_number("€99"), Some(99.0));
        assert_eq!(parse_number("abc"), None);
    }

    #[test]
    fn exact_date_range_beats_spacing() {
        let a = analyze("100\n150", "annual", 0.0, "2020-01-01", "2025-01-01", 0.0, false).unwrap();
        assert_eq!(a.elapsed_years_source, "dates");
        approx(a.elapsed_years, 1827.0 / 365.25); // 5 years incl. one leap day
        assert!(a.cagr > 0.084 && a.cagr < 0.085);
    }

    #[test]
    fn doubling_time_and_target_projection() {
        let a = analyze("100\n200", "annual", 10.0, "", "", 400.0, false).unwrap();
        approx(a.cagr, 2f64.powf(0.1) - 1.0);
        approx(a.doubling_years.unwrap(), 10.0);
        // From 200, reaching 400 is one more doubling = 10 years.
        approx(a.years_to_target.unwrap(), 10.0);
    }

    #[test]
    fn decline_gives_negative_cagr_and_no_doubling() {
        let a = analyze("200\n100", "annual", 1.0, "", "", 0.0, false).unwrap();
        approx(a.cagr, -0.5);
        assert!(a.doubling_years.is_none());
    }

    #[test]
    fn mean_period_growth_exceeds_cagr_when_volatile() {
        let a = analyze("100\n200\n100", "annual", 0.0, "", "", 0.0, false).unwrap();
        approx(a.cagr, 0.0);
        approx(a.mean_period_growth.unwrap(), 0.25); // (+100% + -50%) / 2
        assert_eq!(a.best_period.as_ref().unwrap().0, "2");
    }

    #[test]
    fn err_single_value() {
        let e = analyze("100", "annual", 0.0, "", "", 0.0, false).unwrap_err();
        assert!(e.contains("at least 2 values"), "{e}");
    }

    #[test]
    fn err_zero_or_negative_start() {
        let e = analyze("0\n100", "annual", 0.0, "", "", 0.0, false).unwrap_err();
        assert!(e.contains("positive starting value"), "{e}");
        let e = analyze("100\n-5", "annual", 0.0, "", "", 0.0, false).unwrap_err();
        assert!(e.contains("negative ending value"), "{e}");
    }

    #[test]
    fn err_bad_period_and_output_and_decimals() {
        let e = analyze("1\n2", "fortnightly", 0.0, "", "", 0.0, false).unwrap_err();
        assert!(e.contains("period must be one of"), "{e}");
        let e = summary("1\n2", "annual", 0.0, "", "", 0.0, false, 2, "xml").unwrap_err();
        assert!(e.contains("output must be one of"), "{e}");
        let e = summary("1\n2", "annual", 0.0, "", "", 0.0, false, 11, "summary").unwrap_err();
        assert!(e.contains("decimals must be"), "{e}");
    }

    #[test]
    fn err_non_numeric_line_names_the_line() {
        let e = analyze("100\nnot a number\n200", "annual", 0.0, "", "", 0.0, false).unwrap_err();
        assert!(e.contains("line 2"), "{e}");
    }

    #[test]
    fn err_half_a_date_range() {
        let e = analyze("100\n150", "annual", 0.0, "2020-01-01", "", 0.0, false).unwrap_err();
        assert!(e.contains("BOTH start_date and end_date"), "{e}");
        let e = analyze("100\n150", "annual", 0.0, "2025-01-01", "2020-01-01", 0.0, false).unwrap_err();
        assert!(e.contains("after start_date"), "{e}");
    }

    #[test]
    fn err_too_many_points() {
        let big: Vec<String> = (0..MAX_POINTS + 1).map(|i| (i + 1).to_string()).collect();
        let e = analyze(&big.join("\n"), "annual", 0.0, "", "", 0.0, false).unwrap_err();
        assert!(e.contains("maximum is"), "{e}");
    }

    #[test]
    fn summary_renders_exact_headline() {
        let s = summary("100000\n150000", "annual", 5.0, "", "", 0.0, false, 2, "summary").unwrap();
        assert!(s.starts_with("CAGR: +8.45% per year"), "{s}");
        assert!(s.contains("total growth: +50.00%"), "{s}");
        // Singular/plural and 2-decimal year rounding in the rendered copy.
        assert!(s.contains("elapsed: 5 years (2 data points, 1 period, from the years you set)"), "{s}");
        assert!(s.contains("doubling time at this CAGR: 8.55 years"), "{s}");

        let multi = summary("100\n110\n121\n133.1", "annual", 0.0, "", "", 0.0, false, 2, "summary").unwrap();
        assert!(multi.contains("4 data points, 3 periods"), "{multi}");
        assert!(multi.contains("over 3 annual periods"), "{multi}");
    }

    #[test]
    fn table_and_csv_and_json_outputs() {
        let t = summary("100\n150", "annual", 0.0, "", "", 0.0, false, 2, "table").unwrap();
        assert!(t.starts_with("period"), "{t}");
        assert!(t.contains("+50.00%"), "{t}");
        let c = summary("100\n150", "annual", 0.0, "", "", 0.0, false, 2, "csv").unwrap();
        assert_eq!(c.lines().next().unwrap(), "period,value,change,growth_pct,index");
        assert!(c.contains("2,150,50,50.00,150.00"), "{c}");
        let j = summary("100\n150", "annual", 0.0, "", "", 0.0, false, 2, "json").unwrap();
        let v: serde_json::Value = serde_json::from_str(&j).unwrap();
        assert_eq!(v["cagr"], 0.5);
        assert_eq!(v["count"], 2);
    }

    #[test]
    fn fmt_helpers() {
        assert_eq!(fmt_value(1234.0), "1,234");
        assert_eq!(fmt_value(-1234567.25), "-1,234,567.25");
        assert_eq!(fmt_pct(0.0845, 2), "+8.45%");
    }
}
