//! gizza-ai/drawdown-analyzer core — drawdown analytics for an equity curve or
//! a periodic-returns series: maximum drawdown, every drawdown episode with its
//! decline/recovery/underwater length, the current drawdown, ulcer & pain
//! indexes, and the underwater curve (plus a fixed-width ASCII plot of it).
//!
//! Conventions (fixed, and stated on the page so results are reproducible):
//! - Drawdown at t = value_t / running_peak_t − 1, so it is 0 at a new high and
//!   negative otherwise. The peak is the series' own high-water mark.
//! - An episode STARTS on the first observation below the running peak and ENDS
//!   only when the series closes back at or above that peak. An episode still
//!   below its peak at the last observation is reported as ongoing.
//! - decline = peak→trough, recovery = trough→prior peak, underwater =
//!   decline + recovery (the whole stretch below the peak).
//! - Ulcer index = RMS of the underwater curve over EVERY observation; pain
//!   index = its mean absolute value. Both include at-a-high periods as zeroes.
//! Pure Rust (serde for the output struct, chrono for optional calendar dates).
//! Educational only — not financial advice.

use chrono::{Datelike, Duration, Months, NaiveDate, Weekday};
use serde::Serialize;

/// Hard cap on the number of observations, so a runaway paste fails with a
/// clear message instead of hanging the page.
pub const MAX_POINTS: usize = 20_000;
/// Widest the ASCII underwater plot ever gets, in columns.
const PLOT_WIDTH: usize = 60;
/// Number of rows in the ASCII underwater plot.
const PLOT_ROWS: usize = 8;

/// One drawdown episode: a peak, the trough that followed it, and (when the
/// series got back there) the recovery.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Episode {
    /// 1 = deepest episode in the series.
    pub rank: usize,
    /// Peak-to-trough loss as a decimal (≤ 0; −0.25 = a 25% drawdown).
    pub depth: f64,
    /// 1-based position of the peak observation.
    pub peak_position: usize,
    /// Value at the peak (equity level; 1.0-based for a returns series).
    pub peak_value: f64,
    /// Calendar date of the peak, when the series is dated.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peak_date: Option<String>,
    /// 1-based position of the trough observation.
    pub trough_position: usize,
    /// Value at the trough.
    pub trough_value: f64,
    /// Calendar date of the trough, when the series is dated.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trough_date: Option<String>,
    /// Periods from the peak down to the trough.
    pub decline_periods: usize,
    /// True once the series closed back at or above the peak.
    pub recovered: bool,
    /// 1-based position of the recovery observation, when recovered.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recovery_position: Option<usize>,
    /// Calendar date of the recovery, when dated and recovered.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recovery_date: Option<String>,
    /// Periods from the trough back up to the prior peak, when recovered.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recovery_periods: Option<usize>,
    /// Periods spent below the peak (decline + recovery; to the last
    /// observation while an episode is still ongoing).
    pub underwater_periods: usize,
    /// Gain needed from the trough to get back to the peak (decimal).
    pub required_gain: f64,
}

/// The full drawdown report.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Analysis {
    /// Number of observations parsed.
    pub count: usize,
    /// "equity" or "returns" — how the input was read.
    pub series_type: String,
    /// Frequency label used for durations/dates ("period", "monthly", …).
    pub frequency: String,
    /// Plural noun for one period at this frequency ("periods", "months", …).
    pub period_unit: String,
    /// First equity value (1.0 for a returns series).
    pub start_value: f64,
    /// Last equity value.
    pub end_value: f64,
    /// Total compounded return over the whole series (decimal).
    pub total_return: f64,
    /// Date of the first observation, when the series is dated.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_date: Option<String>,
    /// Date of the last observation, when the series is dated.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_date: Option<String>,
    /// Deepest peak-to-trough loss (decimal ≤ 0).
    pub max_drawdown: f64,
    /// Gain needed from the deepest trough to regain that peak (decimal).
    pub required_gain_to_recover: f64,
    /// Years to earn `required_gain_to_recover` at the assumed CAGR; `None`
    /// when no CAGR was supplied or there was no drawdown.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub estimated_recovery_years: Option<f64>,
    /// Drawdown at the last observation (decimal ≤ 0).
    pub current_drawdown: f64,
    /// Periods the series has been below its peak at the last observation.
    pub current_underwater_periods: usize,
    /// Total number of drawdown episodes found.
    pub episode_count: usize,
    /// Mean episode depth (decimal ≤ 0; 0 when there were none).
    pub average_drawdown: f64,
    /// Mean recovery length over the RECOVERED episodes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub average_recovery_periods: Option<f64>,
    /// Longest single stretch spent below a peak.
    pub longest_underwater_periods: usize,
    /// Share of observations that sat below the running peak (decimal 0..=1).
    pub time_underwater_ratio: f64,
    /// Ulcer index — RMS of the underwater curve (decimal; 0.05 = 5%).
    pub ulcer_index: f64,
    /// Pain index — mean absolute underwater depth (decimal).
    pub pain_index: f64,
    /// The `top_n` deepest episodes, deepest first.
    pub episodes: Vec<Episode>,
    /// Drawdown at every observation (decimal ≤ 0) — the underwater curve.
    pub underwater_curve: Vec<f64>,
    /// The conventions used. Not financial advice.
    pub note: String,
}

fn round6(v: f64) -> f64 {
    (v * 1e6).round() / 1e6
}

/// Frequency → (label, plural unit, singular unit).
fn frequency_parts(freq: &str) -> Option<(&'static str, &'static str, &'static str)> {
    Some(match freq {
        "period" => ("period", "periods", "period"),
        "daily" => ("daily", "days", "day"),
        "trading" => ("trading", "trading days", "trading day"),
        "weekly" => ("weekly", "weeks", "week"),
        "monthly" => ("monthly", "months", "month"),
        "quarterly" => ("quarterly", "quarters", "quarter"),
        "annual" => ("annual", "years", "year"),
        _ => return None,
    })
}

/// Parse one numeric token. `%` is only meaningful for a returns series; a
/// leading currency symbol and `_` digit separators are tolerated.
fn parse_value(tok: &str, returns_mode: bool, position: usize) -> Result<f64, String> {
    let t = tok.trim();
    let (body, is_pct) = match t.strip_suffix('%') {
        Some(rest) => (rest.trim_end(), true),
        None => (t, false),
    };
    if is_pct && !returns_mode {
        return Err(format!(
            "value #{position} is '{t}', a percent — switch the series type to Returns for percent values, or paste plain equity levels"
        ));
    }
    let body = body
        .trim_start_matches(['$', '£', '€'])
        .trim()
        .strip_prefix('+')
        .unwrap_or_else(|| body.trim_start_matches(['$', '£', '€']).trim());
    let cleaned: String = body.chars().filter(|c| *c != '_').collect();
    let n: f64 = cleaned.parse().map_err(|_| {
        format!("value #{position} is '{t}', which is not a number — paste one number per line (or comma-separated), e.g. 10000 or 1.2%")
    })?;
    if !n.is_finite() {
        return Err(format!("value #{position} is '{t}', which is not a finite number"));
    }
    Ok(if is_pct { n / 100.0 } else { n })
}

/// Try to read a leading date field (`2020-01-31` or `2020/01/31`).
fn parse_date_field(s: &str) -> Option<NaiveDate> {
    let t = s.trim();
    if t.len() != 10 {
        return None;
    }
    let norm = t.replace('/', "-");
    NaiveDate::parse_from_str(&norm, "%Y-%m-%d").ok()
}

struct Parsed {
    values: Vec<f64>,
    dates: Option<Vec<NaiveDate>>,
}

/// Split the pasted series into values, optionally with a leading date column
/// (`2020-01-31, 10000` rows). Rows are newline-separated; within a row, values
/// may be separated by commas, semicolons, tabs or spaces.
fn parse_series(text: &str, returns_mode: bool, has_header: bool) -> Result<Parsed, String> {
    let mut lines: Vec<&str> = text.lines().collect();
    if has_header {
        // Drop the first NON-EMPTY line (the column label).
        if let Some(i) = lines.iter().position(|l| !l.trim().is_empty()) {
            lines.remove(i);
        }
    }
    let mut values: Vec<f64> = Vec::new();
    let mut dates: Vec<NaiveDate> = Vec::new();
    let mut dated_rows = 0usize;
    let mut plain_rows = 0usize;

    for line in lines {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let fields: Vec<&str> = line
            .split([',', ';', '\t'])
            .map(|f| f.trim())
            .filter(|f| !f.is_empty())
            .collect();
        if fields.len() >= 2 {
            if let Some(d) = parse_date_field(fields[0]) {
                dated_rows += 1;
                if plain_rows > 0 {
                    return Err(format!(
                        "row {} has a date column but earlier rows do not — use dates on every row, or on none",
                        dated_rows + plain_rows
                    ));
                }
                values.push(parse_value(fields[1], returns_mode, values.len() + 1)?);
                dates.push(d);
                continue;
            }
        }
        // Plain value row: every whitespace/comma-separated token is a value.
        let mut got = false;
        for tok in line.split(|c: char| c.is_whitespace() || c == ',' || c == ';') {
            if tok.trim().is_empty() {
                continue;
            }
            values.push(parse_value(tok, returns_mode, values.len() + 1)?);
            got = true;
        }
        if got {
            plain_rows += 1;
            if dated_rows > 0 {
                return Err(format!(
                    "row {} has no date column but earlier rows do — use dates on every row, or on none",
                    dated_rows + plain_rows
                ));
            }
        }
    }

    if dated_rows > 0 {
        for i in 1..dates.len() {
            if dates[i] < dates[i - 1] {
                return Err(format!(
                    "dates must run oldest-first: row {} ({}) comes before row {} ({})",
                    i + 1,
                    dates[i],
                    i,
                    dates[i - 1]
                ));
            }
        }
        Ok(Parsed { values, dates: Some(dates) })
    } else {
        Ok(Parsed { values, dates: None })
    }
}

fn add_business_days(start: NaiveDate, n: usize) -> Option<NaiveDate> {
    let mut cur = start;
    let mut left = n;
    while left > 0 {
        cur = cur.checked_add_signed(Duration::days(1))?;
        if !matches!(cur.weekday(), Weekday::Sat | Weekday::Sun) {
            left -= 1;
        }
    }
    Some(cur)
}

/// Date of observation `i` (0-based) given a start date and a frequency.
fn date_at(start: NaiveDate, freq: &str, i: usize) -> Option<NaiveDate> {
    let k = i as u32;
    match freq {
        "daily" => start.checked_add_signed(Duration::days(i as i64)),
        "trading" => add_business_days(start, i),
        "weekly" => start.checked_add_signed(Duration::days(7 * i as i64)),
        "monthly" => start.checked_add_months(Months::new(k)),
        "quarterly" => start.checked_add_months(Months::new(3 * k)),
        "annual" => start.checked_add_months(Months::new(12 * k)),
        _ => None,
    }
}

/// Compute the drawdown report.
///
/// * `text` — the series: equity/balance levels, or periodic returns.
/// * `series_type` — `"equity"` or `"returns"`.
/// * `frequency` — `"period"` (unitless) or `daily|trading|weekly|monthly|quarterly|annual`.
/// * `start_date` — `YYYY-MM-DD` of the FIRST observation, or empty. Ignored
///   when the pasted rows already carry a date column.
/// * `has_header` — drop the first line before parsing.
/// * `top_n` — how many episodes to rank (1–20).
/// * `recovery_cagr` — assumed annual growth PERCENT for the time-to-recover
///   estimate; 0 turns the estimate off.
pub fn analyze(
    text: &str,
    series_type: &str,
    frequency: &str,
    start_date: &str,
    has_header: bool,
    top_n: usize,
    recovery_cagr: f64,
) -> Result<Analysis, String> {
    let returns_mode = match series_type {
        "equity" => false,
        "returns" => true,
        other => {
            return Err(format!(
                "series_type must be 'equity' or 'returns', got '{other}'"
            ))
        }
    };
    let (freq_label, unit_plural, unit_singular) = frequency_parts(frequency).ok_or_else(|| {
        format!("frequency must be one of period, daily, trading, weekly, monthly, quarterly, annual — got '{frequency}'")
    })?;
    if !(1..=20).contains(&top_n) {
        return Err(format!("top_n must be between 1 and 20, got {top_n}"));
    }
    if !recovery_cagr.is_finite() || recovery_cagr <= -100.0 {
        return Err(format!(
            "recovery_cagr must be a percent above −100 (0 turns the estimate off), got {recovery_cagr}"
        ));
    }

    let parsed = parse_series(text, returns_mode, has_header)?;
    let raw = parsed.values;
    if raw.len() < 2 {
        return Err(format!(
            "need at least 2 observations to measure a drawdown, got {} — paste one value per line, or comma-separated",
            raw.len()
        ));
    }
    if raw.len() > MAX_POINTS {
        return Err(format!(
            "at most {MAX_POINTS} observations are supported, got {} — trim or downsample the series",
            raw.len()
        ));
    }

    // Build the equity curve.
    let equity: Vec<f64> = if returns_mode {
        let mut eq = Vec::with_capacity(raw.len());
        let mut level = 1.0f64;
        for (i, r) in raw.iter().enumerate() {
            if *r <= -1.0 {
                return Err(format!(
                    "return #{} is {:.4}% — a loss of 100% or more wipes the series out and no drawdown can be measured after it",
                    i + 1,
                    r * 100.0
                ));
            }
            level *= 1.0 + r;
            eq.push(level);
        }
        eq
    } else {
        for (i, v) in raw.iter().enumerate() {
            if *v <= 0.0 {
                return Err(format!(
                    "equity value #{} is {v} — equity levels must be greater than 0 (switch the series type to Returns if these are periodic returns)",
                    i + 1
                ));
            }
        }
        raw.clone()
    };
    let n = equity.len();

    // Dates: from the pasted rows if present, else generated from start_date.
    let dates: Option<Vec<NaiveDate>> = match parsed.dates {
        Some(d) => Some(d),
        None => {
            let sd = start_date.trim();
            if sd.is_empty() {
                None
            } else {
                let base = parse_date_field(sd).ok_or_else(|| {
                    format!("start date must be YYYY-MM-DD, got '{sd}'")
                })?;
                if frequency == "period" {
                    return Err(
                        "a start date needs a real frequency — pick daily, trading, weekly, monthly, quarterly or annual so the observations can be placed on the calendar".into(),
                    );
                }
                let mut out = Vec::with_capacity(n);
                for i in 0..n {
                    out.push(date_at(base, frequency, i).ok_or_else(|| {
                        format!("the series runs past the supported calendar range from {base}")
                    })?);
                }
                Some(out)
            }
        }
    };
    if let Some(d) = &dates {
        if d.len() != n {
            return Err(format!(
                "got {} dates for {n} values — every row needs exactly one date and one value",
                d.len()
            ));
        }
    }
    let date_str = |i: usize| dates.as_ref().map(|d| d[i].to_string());

    // Underwater curve + episodes, walking the running peak.
    let mut curve = Vec::with_capacity(n);
    let mut episodes: Vec<Episode> = Vec::new();
    let mut peak_val = equity[0];
    let mut peak_idx = 0usize;
    let mut trough_val = 0.0f64;
    let mut trough_idx = 0usize;
    let mut in_dd = false;

    let push_episode = |episodes: &mut Vec<Episode>,
                        peak_idx: usize,
                        peak_val: f64,
                        trough_idx: usize,
                        trough_val: f64,
                        recovery: Option<usize>,
                        last_idx: usize| {
        let depth = trough_val / peak_val - 1.0;
        let underwater = match recovery {
            Some(r) => r - peak_idx,
            None => last_idx - peak_idx,
        };
        episodes.push(Episode {
            rank: 0,
            depth: round6(depth),
            peak_position: peak_idx + 1,
            peak_value: round6(peak_val),
            peak_date: None,
            trough_position: trough_idx + 1,
            trough_value: round6(trough_val),
            trough_date: None,
            decline_periods: trough_idx - peak_idx,
            recovered: recovery.is_some(),
            recovery_position: recovery.map(|r| r + 1),
            recovery_date: None,
            recovery_periods: recovery.map(|r| r - trough_idx),
            underwater_periods: underwater,
            required_gain: round6(if depth < 0.0 { 1.0 / (1.0 + depth) - 1.0 } else { 0.0 }),
        });
    };

    for i in 0..n {
        let v = equity[i];
        if !in_dd {
            if v >= peak_val {
                peak_val = v;
                peak_idx = i;
            } else {
                in_dd = true;
                trough_val = v;
                trough_idx = i;
            }
        } else {
            if v < trough_val {
                trough_val = v;
                trough_idx = i;
            }
            if v >= peak_val {
                push_episode(
                    &mut episodes,
                    peak_idx,
                    peak_val,
                    trough_idx,
                    trough_val,
                    Some(i),
                    n - 1,
                );
                in_dd = false;
                peak_val = v;
                peak_idx = i;
            }
        }
        curve.push(round6(v / peak_val - 1.0));
    }
    let current_underwater_periods = if in_dd {
        push_episode(&mut episodes, peak_idx, peak_val, trough_idx, trough_val, None, n - 1);
        n - 1 - peak_idx
    } else {
        0
    };

    let episode_count = episodes.len();
    let max_drawdown = episodes.iter().map(|e| e.depth).fold(0.0f64, f64::min);
    let average_drawdown = if episode_count > 0 {
        episodes.iter().map(|e| e.depth).sum::<f64>() / episode_count as f64
    } else {
        0.0
    };
    let recovered: Vec<f64> = episodes
        .iter()
        .filter_map(|e| e.recovery_periods.map(|r| r as f64))
        .collect();
    let average_recovery_periods = if recovered.is_empty() {
        None
    } else {
        Some(round6(recovered.iter().sum::<f64>() / recovered.len() as f64))
    };
    let longest_underwater_periods = episodes.iter().map(|e| e.underwater_periods).max().unwrap_or(0);
    let underwater_points = curve.iter().filter(|d| **d < 0.0).count();
    let time_underwater_ratio = underwater_points as f64 / n as f64;
    let ulcer_index = (curve.iter().map(|d| d * d).sum::<f64>() / n as f64).sqrt();
    let pain_index = curve.iter().map(|d| d.abs()).sum::<f64>() / n as f64;

    let required_gain_to_recover = if max_drawdown < 0.0 {
        1.0 / (1.0 + max_drawdown) - 1.0
    } else {
        0.0
    };
    let estimated_recovery_years = if recovery_cagr > 0.0 && max_drawdown < 0.0 {
        Some(round6(
            (1.0 + required_gain_to_recover).ln() / (1.0 + recovery_cagr / 100.0).ln(),
        ))
    } else {
        None
    };

    // Rank + trim to the top N deepest, then attach dates.
    episodes.sort_by(|a, b| a.depth.partial_cmp(&b.depth).unwrap_or(core::cmp::Ordering::Equal));
    episodes.truncate(top_n);
    for (i, e) in episodes.iter_mut().enumerate() {
        e.rank = i + 1;
        e.peak_date = date_str(e.peak_position - 1);
        e.trough_date = date_str(e.trough_position - 1);
        e.recovery_date = e.recovery_position.and_then(|p| date_str(p - 1));
    }

    let total_return = equity[n - 1] / equity[0] - 1.0;
    let note = format!(
        "Drawdown is measured against the series' own running peak: 0 at a new high, negative below it. Decline = peak to trough, recovery = trough back to that peak, underwater = both. Ulcer index is the RMS of the underwater curve over all {n} observations and pain index its mean absolute value. Durations are counted in {unit_plural}. Educational only — not financial advice."
    );

    Ok(Analysis {
        count: n,
        series_type: if returns_mode { "returns" } else { "equity" }.to_string(),
        frequency: freq_label.to_string(),
        period_unit: unit_plural.to_string(),
        start_value: round6(equity[0]),
        end_value: round6(equity[n - 1]),
        total_return: round6(total_return),
        start_date: date_str(0),
        end_date: date_str(n - 1),
        max_drawdown: round6(max_drawdown),
        required_gain_to_recover: round6(required_gain_to_recover),
        estimated_recovery_years,
        current_drawdown: curve[n - 1],
        current_underwater_periods,
        episode_count,
        average_drawdown: round6(average_drawdown),
        average_recovery_periods,
        longest_underwater_periods,
        time_underwater_ratio: round6(time_underwater_ratio),
        ulcer_index: round6(ulcer_index),
        pain_index: round6(pain_index),
        episodes,
        underwater_curve: curve,
        note: {
            let _ = unit_singular;
            note
        },
    })
}

fn pct(v: f64) -> String {
    format!("{:.4}%", v * 100.0)
}

fn signed_pct(v: f64) -> String {
    format!("{:+.4}%", v * 100.0)
}

fn num(v: f64) -> String {
    let r = round6(v);
    if r.fract() == 0.0 && r.abs() < 1e15 {
        format!("{}", r as i64)
    } else {
        format!("{r}")
    }
}

/// Duration with its unit, singularized at 1 ("1 month", "3 months").
fn dur(n: usize, unit_plural: &str) -> String {
    if n == 1 {
        let singular = unit_plural.strip_suffix('s').unwrap_or(unit_plural);
        format!("1 {singular}")
    } else {
        format!("{n} {unit_plural}")
    }
}

/// Fixed-width ASCII underwater plot: `#` marks a period below the peak, and
/// each row is a depth band from 0% (top) down to the maximum drawdown.
fn underwater_plot(curve: &[f64], max_depth: f64) -> String {
    let n = curve.len();
    let width = PLOT_WIDTH.min(n);
    // Bucket the curve into `width` columns, keeping the WORST value per bucket.
    let mut cols = Vec::with_capacity(width);
    for c in 0..width {
        let lo = c * n / width;
        let hi = ((c + 1) * n / width).max(lo + 1);
        let worst = curve[lo..hi].iter().cloned().fold(0.0f64, f64::min);
        cols.push(worst.abs());
    }
    let mut out = String::new();
    for r in 0..PLOT_ROWS {
        let band_top = max_depth * r as f64 / PLOT_ROWS as f64;
        let label = format!("{:>9}", format!("-{:.2}%", band_top * 100.0));
        let label = if r == 0 { format!("{:>9}", "0.00%") } else { label };
        let row: String = cols
            .iter()
            .map(|d| if *d > band_top + 1e-12 { '#' } else { ' ' })
            .collect();
        out.push_str(&format!("{label} |{}\n", row.trim_end()));
    }
    out.push_str(&format!(
        "{:>9} +{}\n",
        format!("-{:.2}%", max_depth * 100.0),
        "-".repeat(width)
    ));
    out
}

/// Human-readable report (what the page and CLI text view show).
pub fn summary(
    text: &str,
    series_type: &str,
    frequency: &str,
    start_date: &str,
    has_header: bool,
    top_n: usize,
    recovery_cagr: f64,
) -> Result<String, String> {
    let a = analyze(text, series_type, frequency, start_date, has_header, top_n, recovery_cagr)?;
    let unit = a.period_unit.as_str();
    let mut s = String::new();

    let span = match (&a.start_date, &a.end_date) {
        (Some(f), Some(l)) => format!(", {f} → {l}"),
        _ => String::new(),
    };
    s.push_str(&format!(
        "series: {} {} observations ({}){}\n",
        a.count, a.series_type, a.frequency, span
    ));
    s.push_str(&format!(
        "start {} → end {} (total return {})\n\n",
        num(a.start_value),
        num(a.end_value),
        signed_pct(a.total_return)
    ));

    s.push_str(&format!("max drawdown: {}\n", pct(a.max_drawdown)));
    if let Some(worst) = a.episodes.first() {
        let pd = worst.peak_date.as_ref().map(|d| format!(" {d}")).unwrap_or_default();
        let td = worst.trough_date.as_ref().map(|d| format!(" {d}")).unwrap_or_default();
        s.push_str(&format!(
            "  peak {} at #{}{} → trough {} at #{}{}\n",
            num(worst.peak_value),
            worst.peak_position,
            pd,
            num(worst.trough_value),
            worst.trough_position,
            td
        ));
        s.push_str(&format!(
            "  decline {} · underwater {}\n",
            dur(worst.decline_periods, unit),
            dur(worst.underwater_periods, unit)
        ));
        match (worst.recovery_periods, worst.recovery_position) {
            (Some(rp), Some(pos)) => {
                let rd = worst.recovery_date.as_ref().map(|d| format!(" {d}")).unwrap_or_default();
                s.push_str(&format!("  recovered after {} at #{}{}\n", dur(rp, unit), pos, rd));
            }
            _ => s.push_str("  not recovered yet — still below this peak at the last observation\n"),
        }
    }
    s.push_str(&format!(
        "gain needed to erase it: {}\n",
        signed_pct(a.required_gain_to_recover)
    ));
    if let Some(years) = a.estimated_recovery_years {
        s.push_str(&format!(
            "  ≈ {years:.2} years at {:.2}% a year\n",
            recovery_cagr
        ));
    }
    s.push_str(&format!(
        "current drawdown: {}{}\n\n",
        pct(a.current_drawdown),
        if a.current_drawdown < 0.0 {
            format!(" (underwater {})", dur(a.current_underwater_periods, unit))
        } else {
            " (at a new high)".to_string()
        }
    ));

    s.push_str(&format!(
        "episodes: {} · average depth: {}\n",
        a.episode_count,
        pct(a.average_drawdown)
    ));
    s.push_str(&format!(
        "longest underwater stretch: {} · time underwater: {}\n",
        dur(a.longest_underwater_periods, unit),
        pct(a.time_underwater_ratio)
    ));
    match a.average_recovery_periods {
        Some(r) => s.push_str(&format!("average recovery: {r:.1} {unit}\n")),
        None => s.push_str("average recovery: n/a (no episode has recovered)\n"),
    }
    s.push_str(&format!(
        "ulcer index: {} · pain index: {}\n\n",
        pct(a.ulcer_index),
        pct(a.pain_index)
    ));

    if a.episodes.is_empty() {
        s.push_str("no drawdowns: the series never closed below its running peak.\n\n");
    } else {
        s.push_str(&format!(
            "top {} of {} drawdowns (deepest first):\n",
            a.episodes.len(),
            a.episode_count
        ));
        for e in &a.episodes {
            let pd = e.peak_date.as_ref().map(|d| format!(" {d}")).unwrap_or_default();
            let td = e.trough_date.as_ref().map(|d| format!(" {d}")).unwrap_or_default();
            s.push_str(&format!(
                "  {}. {}  peak #{}{} → trough #{}{}\n",
                e.rank,
                pct(e.depth),
                e.peak_position,
                pd,
                e.trough_position,
                td
            ));
            let status = match (e.recovery_periods, e.recovery_position) {
                (Some(rp), Some(pos)) => {
                    let rd = e.recovery_date.as_ref().map(|d| format!(" {d}")).unwrap_or_default();
                    format!("recovery {} → #{}{}", dur(rp, unit), pos, rd)
                }
                _ => "ongoing".to_string(),
            };
            s.push_str(&format!(
                "     decline {} · {} · underwater {} · needs {}\n",
                dur(e.decline_periods, unit),
                status,
                dur(e.underwater_periods, unit),
                signed_pct(e.required_gain)
            ));
        }
        s.push('\n');
        let max_depth = a.max_drawdown.abs();
        s.push_str(&format!(
            "underwater plot ({} per column, # = below the peak):\n",
            dur(
                (a.count + PLOT_WIDTH.min(a.count) - 1) / PLOT_WIDTH.min(a.count).max(1),
                unit
            )
        ));
        s.push_str(&underwater_plot(&a.underwater_curve, max_depth));
        s.push('\n');
    }

    s.push_str(&a.note);
    s.push('\n');
    Ok(s)
}

/// Surface entry point — the human-readable report the page output box and the
/// CLI text view render. A thin alias for [`summary`] so every wrapper calls one
/// stable name; the chat/CLI JSON surface calls [`analyze`] for the structured
/// form instead.
#[allow(clippy::too_many_arguments)]
pub fn run(
    text: &str,
    series_type: &str,
    frequency: &str,
    start_date: &str,
    has_header: bool,
    top_n: usize,
    recovery_cagr: f64,
) -> Result<String, String> {
    summary(text, series_type, frequency, start_date, has_header, top_n, recovery_cagr)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-6, "expected ~{b}, got {a}");
    }

    #[test]
    fn equity_happy_path() {
        // 100 → 120 (peak) → 90 (trough) → 110 → 130 (new high, recovered).
        let a = analyze("100, 120, 90, 110, 130", "equity", "period", "", false, 5, 0.0).unwrap();
        assert_eq!(a.count, 5);
        assert_eq!(a.episode_count, 1);
        approx(a.max_drawdown, -0.25);
        approx(a.total_return, 0.3);
        approx(a.required_gain_to_recover, 1.0 / 0.75 - 1.0);
        assert_eq!(a.current_drawdown, 0.0);
        let e = &a.episodes[0];
        assert_eq!((e.peak_position, e.trough_position), (2, 3));
        assert_eq!(e.decline_periods, 1);
        assert_eq!(e.recovery_periods, Some(2));
        assert_eq!(e.underwater_periods, 3);
        assert_eq!(e.recovery_position, Some(5));
        assert!(e.recovered);
        approx(a.time_underwater_ratio, 0.4);
        approx(a.pain_index, (0.25 + 0.083333) / 5.0);
    }

    #[test]
    fn returns_series_matches_equivalent_equity() {
        // +20%, -25%, +22.222…%, +18.1818…% reproduces 100→120→90→110→130.
        let from_returns =
            analyze("20%, -25%, 22.222222%, 18.181818%", "returns", "period", "", false, 5, 0.0)
                .unwrap();
        assert!((from_returns.max_drawdown + 0.25).abs() < 1e-5);
        assert_eq!(from_returns.episode_count, 1);
        assert_eq!(from_returns.count, 4);
        // Decimal and percent forms agree.
        let dec = analyze("0.2, -0.25", "returns", "period", "", false, 5, 0.0).unwrap();
        let pc = analyze("20%\n-25%", "returns", "period", "", false, 5, 0.0).unwrap();
        assert_eq!(dec.max_drawdown, pc.max_drawdown);
    }

    #[test]
    fn ongoing_drawdown_is_not_reported_as_recovered() {
        let a = analyze("100\n90\n80\n85", "equity", "monthly", "", false, 5, 0.0).unwrap();
        approx(a.max_drawdown, -0.2);
        approx(a.current_drawdown, -0.15);
        assert_eq!(a.current_underwater_periods, 3);
        let e = &a.episodes[0];
        assert!(!e.recovered);
        assert_eq!(e.recovery_periods, None);
        assert_eq!(e.underwater_periods, 3);
        assert_eq!(a.average_recovery_periods, None);
    }

    #[test]
    fn two_episodes_are_ranked_by_depth() {
        // A shallow dip, a full recovery, then a deeper dip.
        let a = analyze("100,95,100,110,77,110", "equity", "period", "", false, 5, 0.0).unwrap();
        assert_eq!(a.episode_count, 2);
        assert_eq!(a.episodes[0].rank, 1);
        approx(a.episodes[0].depth, -0.3);
        approx(a.episodes[1].depth, -0.05);
        approx(a.average_drawdown, -0.175);
        assert_eq!(a.average_recovery_periods, Some(1.0));
        assert_eq!(a.longest_underwater_periods, 2);
    }

    #[test]
    fn top_n_trims_the_ranked_list_but_not_the_count() {
        let a = analyze("100,90,100,80,100,70,100", "equity", "period", "", false, 1, 0.0).unwrap();
        assert_eq!(a.episode_count, 3);
        assert_eq!(a.episodes.len(), 1);
        approx(a.episodes[0].depth, -0.3);
    }

    #[test]
    fn no_drawdown_series() {
        let a = analyze("100 101 102", "equity", "period", "", false, 5, 0.0).unwrap();
        assert_eq!(a.episode_count, 0);
        assert_eq!(a.max_drawdown, 0.0);
        assert_eq!(a.ulcer_index, 0.0);
        assert_eq!(a.required_gain_to_recover, 0.0);
        assert!(summary("100 101 102", "equity", "period", "", false, 5, 0.0)
            .unwrap()
            .contains("never closed below its running peak"));
    }

    #[test]
    fn dated_rows_supply_the_calendar() {
        let a = analyze(
            "2020-01-31,100\n2020-02-29,120\n2020-03-31,90\n2020-04-30,130",
            "equity",
            "monthly",
            "",
            false,
            5,
            0.0,
        )
        .unwrap();
        assert_eq!(a.start_date.as_deref(), Some("2020-01-31"));
        assert_eq!(a.end_date.as_deref(), Some("2020-04-30"));
        let e = &a.episodes[0];
        assert_eq!(e.peak_date.as_deref(), Some("2020-02-29"));
        assert_eq!(e.trough_date.as_deref(), Some("2020-03-31"));
        assert_eq!(e.recovery_date.as_deref(), Some("2020-04-30"));
    }

    #[test]
    fn start_date_plus_frequency_generates_dates() {
        let monthly = analyze("100,90,100", "equity", "monthly", "2021-01-15", false, 5, 0.0).unwrap();
        assert_eq!(monthly.end_date.as_deref(), Some("2021-03-15"));
        let weekly = analyze("100,90,100", "equity", "weekly", "2021-01-15", false, 5, 0.0).unwrap();
        assert_eq!(weekly.end_date.as_deref(), Some("2021-01-29"));
        // Trading days skip the weekend: Fri 2021-01-15 + 2 → Tue 2021-01-19.
        let trading = analyze("100,90,100", "equity", "trading", "2021-01-15", false, 5, 0.0).unwrap();
        assert_eq!(trading.end_date.as_deref(), Some("2021-01-19"));
        let quarterly =
            analyze("100,90,100", "equity", "quarterly", "2021-01-15", false, 5, 0.0).unwrap();
        assert_eq!(quarterly.end_date.as_deref(), Some("2021-07-15"));
        let annual = analyze("100,90,100", "equity", "annual", "2021-01-15", false, 5, 0.0).unwrap();
        assert_eq!(annual.end_date.as_deref(), Some("2023-01-15"));
        let daily = analyze("100,90,100", "equity", "daily", "2021-01-15", false, 5, 0.0).unwrap();
        assert_eq!(daily.end_date.as_deref(), Some("2021-01-17"));
    }

    #[test]
    fn recovery_cagr_estimates_years() {
        let a = analyze("100,50", "equity", "period", "", false, 5, 15.0).unwrap();
        approx(a.max_drawdown, -0.5);
        approx(a.required_gain_to_recover, 1.0);
        // ln(2)/ln(1.15) ≈ 4.959 years.
        let years = a.estimated_recovery_years.unwrap();
        assert!((years - 4.959).abs() < 0.01, "got {years}");
        assert!(analyze("100,50", "equity", "period", "", false, 5, 0.0)
            .unwrap()
            .estimated_recovery_years
            .is_none());
    }

    #[test]
    fn ulcer_and_pain_indexes() {
        // Curve: 0, 0, -0.25, -1/12, 0 for 100,120,90,110,130.
        let a = analyze("100,120,90,110,130", "equity", "period", "", false, 5, 0.0).unwrap();
        let dd = [0.0, 0.0, -0.25, 90.0f64.max(0.0) * 0.0 + (110.0 / 120.0 - 1.0), 0.0];
        let expect_ulcer = (dd.iter().map(|d| d * d).sum::<f64>() / 5.0).sqrt();
        approx(a.ulcer_index, round6(expect_ulcer));
        approx(a.pain_index, round6(dd.iter().map(|d| d.abs()).sum::<f64>() / 5.0));
    }

    #[test]
    fn header_line_is_skipped() {
        let a = analyze("balance\n100\n80\n100", "equity", "period", "", true, 5, 0.0).unwrap();
        assert_eq!(a.count, 3);
        approx(a.max_drawdown, -0.2);
    }

    #[test]
    fn error_on_too_few_values() {
        let e = analyze("100", "equity", "period", "", false, 5, 0.0).unwrap_err();
        assert!(e.contains("at least 2 observations"), "{e}");
    }

    #[test]
    fn error_on_non_numeric_value() {
        let e = analyze("100, abc, 90", "equity", "period", "", false, 5, 0.0).unwrap_err();
        assert!(e.contains("value #2") && e.contains("not a number"), "{e}");
    }

    #[test]
    fn error_on_percent_in_equity_mode() {
        let e = analyze("100, 5%", "equity", "period", "", false, 5, 0.0).unwrap_err();
        assert!(e.contains("switch the series type to Returns"), "{e}");
    }

    #[test]
    fn error_on_non_positive_equity() {
        let e = analyze("100, 0, 90", "equity", "period", "", false, 5, 0.0).unwrap_err();
        assert!(e.contains("greater than 0"), "{e}");
    }

    #[test]
    fn error_on_total_loss_return() {
        let e = analyze("0.1, -100%", "returns", "period", "", false, 5, 0.0).unwrap_err();
        assert!(e.contains("wipes the series out"), "{e}");
    }

    #[test]
    fn error_on_bad_enum_and_top_n() {
        assert!(analyze("100,90", "prices", "period", "", false, 5, 0.0)
            .unwrap_err()
            .contains("series_type must be"));
        assert!(analyze("100,90", "equity", "hourly", "", false, 5, 0.0)
            .unwrap_err()
            .contains("frequency must be one of"));
        assert!(analyze("100,90", "equity", "period", "", false, 0, 0.0)
            .unwrap_err()
            .contains("top_n must be between 1 and 20"));
    }

    #[test]
    fn error_on_start_date_without_frequency() {
        let e = analyze("100,90", "equity", "period", "2020-01-01", false, 5, 0.0).unwrap_err();
        assert!(e.contains("needs a real frequency"), "{e}");
        let bad = analyze("100,90", "equity", "monthly", "01/2020", false, 5, 0.0).unwrap_err();
        assert!(bad.contains("start date must be YYYY-MM-DD"), "{bad}");
    }

    #[test]
    fn error_on_mixed_and_out_of_order_dates() {
        let mixed = analyze("2020-01-31,100\n90", "equity", "monthly", "", false, 5, 0.0).unwrap_err();
        assert!(mixed.contains("date column"), "{mixed}");
        let order = analyze(
            "2020-02-29,100\n2020-01-31,90",
            "equity",
            "monthly",
            "",
            false,
            5,
            0.0,
        )
        .unwrap_err();
        assert!(order.contains("oldest-first"), "{order}");
    }

    #[test]
    fn cap_boundary() {
        let ok: String = (0..MAX_POINTS).map(|i| format!("{}\n", 100 + i % 7)).collect();
        assert_eq!(analyze(&ok, "equity", "period", "", false, 5, 0.0).unwrap().count, MAX_POINTS);
        let over: String = (0..MAX_POINTS + 1).map(|i| format!("{}\n", 100 + i % 7)).collect();
        let e = analyze(&over, "equity", "period", "", false, 5, 0.0).unwrap_err();
        assert!(e.contains("at most 20000 observations"), "{e}");
    }

    #[test]
    fn summary_renders_the_plot_and_key_lines() {
        let s = summary("100, 120, 90, 110, 130", "equity", "period", "", false, 5, 0.0).unwrap();
        assert!(s.contains("max drawdown: -25.0000%"), "{s}");
        assert!(s.contains("gain needed to erase it: +33.3333%"), "{s}");
        assert!(s.contains("underwater plot"), "{s}");
        assert!(s.contains('#'), "{s}");
        assert!(s.contains("not financial advice"), "{s}");
    }

    #[test]
    fn run_is_the_human_readable_report() {
        let series = "2020-01-31,100\n2020-02-29,120\n2020-03-31,90\n2020-04-30,130";
        let r = run(series, "equity", "monthly", "", false, 5, 12.0).unwrap();
        assert_eq!(
            r,
            summary(series, "equity", "monthly", "", false, 5, 12.0).unwrap()
        );
        assert!(r.contains("max drawdown: -25.0000%"), "{r}");
        assert!(r.contains("2020-02-29"), "{r}");
        assert!(r.contains("years at 12.00% a year"), "{r}");
    }

    #[test]
    fn run_reports_errors_as_messages() {
        let e = run("100", "equity", "period", "", false, 5, 0.0).unwrap_err();
        assert!(e.contains("at least 2 observations"), "{e}");
    }
}
