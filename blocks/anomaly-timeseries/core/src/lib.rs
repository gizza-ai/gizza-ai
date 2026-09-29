//! gizza-ai/anomaly-timeseries core — flag anomalous points in an ordered numeric
//! series using rolling-baseline and seasonal-deviation rules. Pure Rust,
//! dependency-free besides serde for the output structs.
//!
//! Design notes (the things that make the verdicts defensible):
//!
//! * Every baseline is **leave-one-out**: the point under test never contributes
//!   to its own expected value or spread, so a single large spike cannot hide
//!   inside the mean it is being compared against.
//! * `center = false` (the default) is the live-monitoring reading: only points
//!   BEFORE `i` feed the baseline. `center = true` is the retrospective reading:
//!   a symmetric window around `i` for the rolling rules, and every other cycle
//!   for the seasonal rule.
//! * `threshold` is what flags an anomaly; `warn_threshold` only labels
//!   near-misses `warning` so a watch band never inflates the anomaly count.
//! * `tolerance` is an absolute deadband in the series' own units. A point within
//!   it of its expected value is never flagged however large its score — this is
//!   the fix for a near-flat baseline turning rounding noise into "anomalies".
//! * A baseline with zero spread has no finite score. Rather than emit an
//!   infinity JSON cannot represent, such a point gets `score: null` and is
//!   flagged `critical` only if it clears the tolerance and direction filters;
//!   `summary.flat_baseline` counts them.

use serde::Serialize;

/// Fewest values we can do anything with (a point plus two baseline peers).
pub const MIN_POINTS: usize = 3;
/// Max series length we'll process, to bound work on hostile input.
pub const MAX_POINTS: usize = 20_000;
/// Max rolling window length.
pub const MAX_WINDOW: i64 = 1_000;
/// Max seasonal period.
pub const MAX_PERIOD: i64 = 1_000;
/// Max threshold / tolerance, to keep the schema bounded.
pub const MAX_THRESHOLD: f64 = 1_000.0;
/// Max decimal places for rounding.
pub const MAX_DECIMALS: i64 = 10;
/// Scale that makes a median-absolute-deviation comparable with a standard
/// deviation on normal data (1 / 0.6745).
const MAD_TO_SD: f64 = 1.4826;

// ---------------------------------------------------------------------------
// options
// ---------------------------------------------------------------------------

/// Which rule (or rules) decides whether a point is anomalous.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    /// Rolling mean ± SD of the surrounding window.
    RollingZ,
    /// Rolling median ± scaled MAD — robust when the window itself has spikes.
    RollingMad,
    /// Mean ± SD of the points at the same position in other cycles.
    SeasonalZ,
    /// Worst of the rolling z-score and seasonal rules.
    Combined,
}

impl Method {
    pub fn parse(s: &str) -> Result<Method, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "rolling_z" | "rolling" | "rolling-z" | "z" | "zscore" | "z_score" => {
                Ok(Method::RollingZ)
            }
            "rolling_mad" | "rolling-mad" | "mad" | "robust" => Ok(Method::RollingMad),
            "seasonal_z" | "seasonal-z" | "seasonal" => Ok(Method::SeasonalZ),
            "combined" | "both" => Ok(Method::Combined),
            other => Err(format!(
                "method must be one of rolling_z, rolling_mad, seasonal_z, combined (got {other:?})"
            )),
        }
    }

    fn name(self) -> &'static str {
        match self {
            Method::RollingZ => "rolling_z",
            Method::RollingMad => "rolling_mad",
            Method::SeasonalZ => "seasonal_z",
            Method::Combined => "combined",
        }
    }

    fn uses_rolling(self) -> bool {
        matches!(
            self,
            Method::RollingZ | Method::RollingMad | Method::Combined
        )
    }

    fn uses_seasonal(self) -> bool {
        matches!(self, Method::SeasonalZ | Method::Combined)
    }

    fn robust_rolling(self) -> bool {
        self == Method::RollingMad
    }
}

/// Which side of the baseline counts as an anomaly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Both,
    Above,
    Below,
}

impl Direction {
    pub fn parse(s: &str) -> Result<Direction, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "both" | "two-sided" => Ok(Direction::Both),
            "above" | "up" | "high" => Ok(Direction::Above),
            "below" | "down" | "low" => Ok(Direction::Below),
            other => Err(format!(
                "direction must be one of both, above, below (got {other:?})"
            )),
        }
    }

    fn name(self) -> &'static str {
        match self {
            Direction::Both => "both",
            Direction::Above => "above",
            Direction::Below => "below",
        }
    }

    /// Does a deviation of this sign count at all?
    fn allows(self, deviation: f64) -> bool {
        match self {
            Direction::Both => true,
            Direction::Above => deviation > 0.0,
            Direction::Below => deviation < 0.0,
        }
    }
}

/// How the result is rendered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OutputFormat {
    Json,
    Table,
    Csv,
}

impl OutputFormat {
    fn parse(s: &str) -> Result<OutputFormat, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "json" => Ok(OutputFormat::Json),
            "table" | "text" => Ok(OutputFormat::Table),
            "csv" => Ok(OutputFormat::Csv),
            other => Err(format!(
                "output must be one of json, table, csv (got {other:?})"
            )),
        }
    }
}

/// Every tunable, mirroring the descriptor params one-for-one.
#[derive(Debug, Clone, PartialEq)]
pub struct Options {
    pub method: String,
    pub window: u32,
    pub min_periods: u32,
    pub period: u32,
    pub threshold: f64,
    pub warn_threshold: f64,
    pub tolerance: f64,
    pub direction: String,
    pub center: bool,
    pub only_anomalies: bool,
    pub decimals: u32,
    pub output: String,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            method: "rolling_z".into(),
            window: 12,
            min_periods: 0,
            period: 7,
            threshold: 3.0,
            warn_threshold: 2.0,
            tolerance: 0.0,
            direction: "both".into(),
            center: false,
            only_anomalies: false,
            decimals: 6,
            output: "json".into(),
        }
    }
}

// ---------------------------------------------------------------------------
// output shape
// ---------------------------------------------------------------------------

/// One scored row of the series.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Point {
    /// 1-based row number in the pasted series.
    pub index: usize,
    /// The row's label (date/period name) when the input carried one.
    pub label: String,
    pub value: f64,
    /// Baseline centre (rolling mean/median, or the seasonal mean). `null` when
    /// the point had too little history to score.
    pub expected: Option<f64>,
    /// `value - expected`.
    pub deviation: Option<f64>,
    /// Deviation in baseline spreads. `null` when unscored, or when the baseline
    /// is flat (zero spread) and no finite score exists.
    pub score: Option<f64>,
    /// `expected - threshold * spread` — the normal band's lower edge.
    pub lower: Option<f64>,
    /// `expected + threshold * spread` — the normal band's upper edge.
    pub upper: Option<f64>,
    /// `unscored` | `normal` | `warning` | `critical`.
    pub severity: &'static str,
    /// True only for `critical` — the points that crossed `threshold`.
    pub anomaly: bool,
    /// Which rule produced the reported numbers: `rolling`, `seasonal`, or empty
    /// when the point is unscored.
    pub rule: &'static str,
    /// Rules that reached at least `warn_threshold` here (only ever more than one
    /// under `method = combined`).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub rules_fired: Vec<&'static str>,
    /// How many peer points backed the reported baseline.
    pub baseline_n: Option<usize>,
}

/// Counts over the whole run.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Summary {
    /// Values parsed from the input.
    pub points: usize,
    /// Points that had enough history for a verdict.
    pub scored: usize,
    /// Points skipped for want of history (warm-up).
    pub unscored: usize,
    /// Points at or beyond `threshold` (`severity = critical`).
    pub anomalies: usize,
    /// Points in the `warn_threshold`..`threshold` watch band.
    pub warnings: usize,
    /// Scored points inside the band.
    pub normal: usize,
    /// `anomalies / scored * 100`.
    pub anomaly_rate: f64,
    /// Points whose baseline had zero spread, so no finite score exists.
    pub flat_baseline: usize,
    /// Largest |score| seen, and its 1-based row number.
    pub max_score: Option<f64>,
    pub max_score_index: Option<usize>,
    /// 1-based row numbers of the first and last anomaly.
    pub first_anomaly_index: Option<usize>,
    pub last_anomaly_index: Option<usize>,
    /// Whole-series mean and sample standard deviation, for context.
    pub mean: f64,
    pub std_dev: f64,
}

/// The full result, serialized as the tool's JSON output.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Report {
    /// Number of input values.
    pub count: usize,
    pub method: &'static str,
    /// Rolling window length (omitted for the purely seasonal rule).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window: Option<usize>,
    /// Baseline points required before a rolling verdict is given.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_periods: Option<usize>,
    /// Seasonal cycle length (omitted when no seasonal rule ran).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub period: Option<usize>,
    pub threshold: f64,
    pub warn_threshold: f64,
    pub tolerance: f64,
    pub direction: &'static str,
    /// False = only earlier points feed a baseline; true = symmetric/retrospective.
    pub center: bool,
    /// Every row, or only the warning/critical rows when `only_anomalies` is set.
    pub points: Vec<Point>,
    /// 1-based row numbers of the anomalies.
    pub anomaly_indices: Vec<usize>,
    /// The anomalies' values, aligned with `anomaly_indices`.
    pub anomaly_values: Vec<f64>,
    /// The anomalies' scores, aligned with `anomaly_indices` (`null` on a flat baseline).
    pub anomaly_scores: Vec<Option<f64>>,
    pub summary: Summary,
    /// One-paragraph plain-language reading of the run.
    pub interpretation: String,
}

// ---------------------------------------------------------------------------
// input parsing
// ---------------------------------------------------------------------------

fn parse_num(s: &str) -> Option<f64> {
    let t = s.trim().trim_matches('"').trim();
    if t.is_empty() {
        return None;
    }
    let t = t.replace('_', "");
    t.parse::<f64>().ok().filter(|v| v.is_finite())
}

fn tokens_of(line: &str) -> Vec<&str> {
    line.split(|c: char| c == ',' || c == ';' || c == '\t' || c.is_whitespace())
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .collect()
}

/// Parse the pasted series into values plus (possibly empty) labels.
///
/// Accepts one number per line, `label,value` rows (the value is the last numeric
/// token, so `2026-01-03, 118` keeps its date), or a single line of separated
/// values. A leading non-numeric header row/token is skipped.
fn parse_series(data: &str) -> Result<(Vec<f64>, Vec<String>), String> {
    let lines: Vec<(usize, &str)> = data
        .lines()
        .enumerate()
        .map(|(i, l)| (i + 1, l.trim_end_matches('\r').trim()))
        .filter(|(_, l)| !l.is_empty())
        .collect();
    if lines.is_empty() {
        return Err(format!(
            "no data found: paste at least {MIN_POINTS} numbers, one per line, as `label,value` rows, or separated by commas/spaces"
        ));
    }

    let mut values: Vec<f64> = Vec::new();
    let mut labels: Vec<String> = Vec::new();

    if lines.len() == 1 {
        for t in tokens_of(lines[0].1) {
            match parse_num(t) {
                Some(v) => values.push(v),
                None if values.is_empty() => continue, // leading header token
                None => return Err(format!("line 1: '{t}' is not a number")),
            }
        }
        labels = vec![String::new(); values.len()];
    } else {
        for (lineno, line) in lines {
            let toks = tokens_of(line);
            if toks.is_empty() {
                continue;
            }
            if toks.len() == 1 {
                match parse_num(toks[0]) {
                    Some(v) => {
                        values.push(v);
                        labels.push(String::new());
                    }
                    None if values.is_empty() => continue, // header row
                    None => return Err(format!("line {lineno}: '{}' is not a number", toks[0])),
                }
            } else {
                // `label, value` — the label may contain separators, so the value
                // is the LAST numeric token and the label is what precedes it.
                match toks.iter().rposition(|t| parse_num(t).is_some()) {
                    Some(idx) if idx > 0 => {
                        values.push(parse_num(toks[idx]).unwrap());
                        labels.push(toks[..idx].join(" "));
                    }
                    Some(idx) => {
                        // The number came first: treat the rest as a trailing label.
                        values.push(parse_num(toks[idx]).unwrap());
                        labels.push(toks[1..].join(" "));
                    }
                    None if values.is_empty() => continue, // header row
                    None => {
                        return Err(format!(
                            "line {lineno}: no number found in '{}'",
                            toks.join(" ")
                        ))
                    }
                }
            }
        }
    }

    if values.len() < MIN_POINTS {
        return Err(format!(
            "needs at least {MIN_POINTS} numeric values, got {}",
            values.len()
        ));
    }
    if values.len() > MAX_POINTS {
        return Err(format!(
            "too many values: {} exceeds the {MAX_POINTS} limit",
            values.len()
        ));
    }
    labels.resize(values.len(), String::new());
    Ok((values, labels))
}

// ---------------------------------------------------------------------------
// statistics
// ---------------------------------------------------------------------------

fn mean_of(xs: &[f64]) -> f64 {
    xs.iter().sum::<f64>() / xs.len() as f64
}

/// Sample (n-1) standard deviation; 0 for a single value.
fn sd_of(xs: &[f64]) -> f64 {
    if xs.len() < 2 {
        return 0.0;
    }
    let m = mean_of(xs);
    let var = xs.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / (xs.len() - 1) as f64;
    var.max(0.0).sqrt()
}

fn median_of(xs: &mut [f64]) -> f64 {
    xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = xs.len();
    if n % 2 == 1 {
        xs[n / 2]
    } else {
        (xs[n / 2 - 1] + xs[n / 2]) / 2.0
    }
}

/// A leave-one-out baseline: where the point was expected to sit and how much
/// spread the comparison peers showed.
#[derive(Debug, Clone, Copy)]
struct Baseline {
    center: f64,
    spread: f64,
    n: usize,
}

fn baseline_of(vals: &[f64], robust: bool) -> Baseline {
    if robust {
        let mut v = vals.to_vec();
        let med = median_of(&mut v);
        let mut dev: Vec<f64> = vals.iter().map(|x| (x - med).abs()).collect();
        let mad = median_of(&mut dev);
        Baseline {
            center: med,
            spread: mad * MAD_TO_SD,
            n: vals.len(),
        }
    } else {
        Baseline {
            center: mean_of(vals),
            spread: sd_of(vals),
            n: vals.len(),
        }
    }
}

/// Rolling baseline for row `i`, always excluding row `i` itself.
fn rolling_baseline(
    values: &[f64],
    i: usize,
    window: usize,
    min_periods: usize,
    center: bool,
    robust: bool,
) -> Option<Baseline> {
    let mut peers: Vec<f64> = Vec::new();
    if center {
        let half = (window / 2).max(1);
        let lo = i.saturating_sub(half);
        let hi = (i + half).min(values.len() - 1);
        for j in lo..=hi {
            if j != i {
                peers.push(values[j]);
            }
        }
    } else {
        let lo = i.saturating_sub(window);
        for j in lo..i {
            peers.push(values[j]);
        }
    }
    if peers.len() < min_periods.max(2) {
        return None;
    }
    Some(baseline_of(&peers, robust))
}

/// Seasonal baseline for row `i`: the points at the same position in other cycles.
fn seasonal_baseline(values: &[f64], i: usize, period: usize, center: bool) -> Option<Baseline> {
    let phase = i % period;
    let mut peers: Vec<f64> = Vec::new();
    let mut j = phase;
    while j < values.len() {
        if j != i && (center || j < i) {
            peers.push(values[j]);
        }
        j += period;
    }
    if peers.len() < 2 {
        return None;
    }
    Some(baseline_of(&peers, false))
}

fn round_to(v: f64, decimals: u32) -> f64 {
    if !v.is_finite() {
        return v;
    }
    let f = 10f64.powi(decimals as i32);
    let scaled = v * f;
    if !scaled.is_finite() {
        return v;
    }
    let r = scaled.round() / f;
    if r == 0.0 {
        0.0 // normalise -0.0
    } else {
        r
    }
}

// ---------------------------------------------------------------------------
// verdicts
// ---------------------------------------------------------------------------

const SEV_UNSCORED: &str = "unscored";
const SEV_NORMAL: &str = "normal";
const SEV_WARNING: &str = "warning";
const SEV_CRITICAL: &str = "critical";

fn sev_rank(sev: &str) -> u8 {
    match sev {
        SEV_CRITICAL => 3,
        SEV_WARNING => 2,
        SEV_NORMAL => 1,
        _ => 0,
    }
}

/// One rule's reading of one point.
#[derive(Debug, Clone, Copy)]
struct Verdict {
    rule: &'static str,
    base: Baseline,
    deviation: f64,
    /// `None` when the baseline is flat (zero spread).
    score: Option<f64>,
    severity: &'static str,
}

impl Verdict {
    /// Sort key for "which reading is the more alarming one".
    fn weight(&self) -> (u8, f64) {
        let mag = match self.score {
            Some(s) => s.abs(),
            None => f64::MAX, // a flat baseline outranks any finite score
        };
        (sev_rank(self.severity), mag)
    }
}

fn judge(
    rule: &'static str,
    base: Baseline,
    value: f64,
    threshold: f64,
    warn: f64,
    tolerance: f64,
    direction: Direction,
) -> Verdict {
    let deviation = value - base.center;
    let score = if base.spread > 0.0 {
        Some(deviation / base.spread)
    } else {
        None
    };
    let eligible = direction.allows(deviation) && deviation.abs() > tolerance;
    let severity = if !eligible {
        SEV_NORMAL
    } else {
        match score {
            Some(s) if s.abs() >= threshold => SEV_CRITICAL,
            Some(s) if warn > 0.0 && s.abs() >= warn => SEV_WARNING,
            Some(_) => SEV_NORMAL,
            // No finite score: a flat baseline plus a deviation that cleared the
            // tolerance is a genuine break, reported without a score.
            None => SEV_CRITICAL,
        }
    };
    Verdict {
        rule,
        base,
        deviation,
        score,
        severity,
    }
}

// ---------------------------------------------------------------------------
// public API
// ---------------------------------------------------------------------------

/// Score the series and render it as JSON, a text table, or CSV.
pub fn render(series: &str, opts: &Options) -> Result<String, String> {
    let report = analyze(series, opts)?;
    match OutputFormat::parse(&opts.output)? {
        OutputFormat::Json => serde_json::to_string_pretty(&report).map_err(|e| e.to_string()),
        OutputFormat::Table => Ok(render_table(&report)),
        OutputFormat::Csv => Ok(render_csv(&report)),
    }
}

/// Score the series, returning the structured report.
pub fn analyze(series: &str, opts: &Options) -> Result<Report, String> {
    let method = Method::parse(&opts.method)?;
    let direction = Direction::parse(&opts.direction)?;
    OutputFormat::parse(&opts.output)?;

    let window = opts.window as i64;
    if window < 2 || window > MAX_WINDOW {
        return Err(format!(
            "window must be between 2 and {MAX_WINDOW} (got {window})"
        ));
    }
    let window = window as usize;
    let min_periods = if opts.min_periods == 0 {
        window
    } else {
        opts.min_periods as usize
    };
    if min_periods > window {
        return Err(format!(
            "min_periods ({min_periods}) cannot exceed window ({window}); use 0 to require the full window"
        ));
    }
    if !(0.0..=MAX_THRESHOLD).contains(&opts.threshold) || !opts.threshold.is_finite() {
        return Err(format!(
            "threshold must be between 0 and {MAX_THRESHOLD} (got {})",
            opts.threshold
        ));
    }
    if !(0.0..=MAX_THRESHOLD).contains(&opts.warn_threshold) || !opts.warn_threshold.is_finite() {
        return Err(format!(
            "warn_threshold must be between 0 and {MAX_THRESHOLD} (got {})",
            opts.warn_threshold
        ));
    }
    if opts.warn_threshold > opts.threshold {
        return Err(format!(
            "warn_threshold ({}) must not exceed threshold ({}); set warn_threshold to 0 to switch the watch band off",
            opts.warn_threshold, opts.threshold
        ));
    }
    if !opts.tolerance.is_finite() || opts.tolerance < 0.0 {
        return Err(format!(
            "tolerance must be 0 or more, in the same units as the series (got {})",
            opts.tolerance
        ));
    }
    if opts.decimals as i64 > MAX_DECIMALS {
        return Err(format!(
            "decimals must be between 0 and {MAX_DECIMALS} (got {})",
            opts.decimals
        ));
    }
    let decimals = opts.decimals;

    let (values, labels) = parse_series(series)?;
    let n = values.len();

    let period = if method.uses_seasonal() {
        let p = opts.period as i64;
        if p < 2 || p > MAX_PERIOD {
            return Err(format!(
                "period must be between 2 and {MAX_PERIOD} for the {} rule (got {p})",
                method.name()
            ));
        }
        let p = p as usize;
        if n < 2 * p + 1 {
            return Err(format!(
                "the seasonal rule needs at least 3 points at each position in the cycle: period {p} needs {} values, got {n}. Use method=rolling_z for a series this short.",
                2 * p + 1
            ));
        }
        Some(p)
    } else {
        None
    };

    let mut points: Vec<Point> = Vec::with_capacity(n);
    let mut anomaly_indices: Vec<usize> = Vec::new();
    let mut anomaly_values: Vec<f64> = Vec::new();
    let mut anomaly_scores: Vec<Option<f64>> = Vec::new();
    let (mut scored, mut warnings, mut normal, mut flat_baseline) =
        (0usize, 0usize, 0usize, 0usize);
    let mut max_score: Option<f64> = None;
    let mut max_score_index: Option<usize> = None;

    for i in 0..n {
        let value = values[i];
        let mut verdicts: Vec<Verdict> = Vec::new();
        if method.uses_rolling() {
            if let Some(b) = rolling_baseline(
                &values,
                i,
                window,
                min_periods,
                opts.center,
                method.robust_rolling(),
            ) {
                verdicts.push(judge(
                    "rolling",
                    b,
                    value,
                    opts.threshold,
                    opts.warn_threshold,
                    opts.tolerance,
                    direction,
                ));
            }
        }
        if let Some(p) = period {
            if let Some(b) = seasonal_baseline(&values, i, p, opts.center) {
                verdicts.push(judge(
                    "seasonal",
                    b,
                    value,
                    opts.threshold,
                    opts.warn_threshold,
                    opts.tolerance,
                    direction,
                ));
            }
        }

        let row = if verdicts.is_empty() {
            Point {
                index: i + 1,
                label: labels[i].clone(),
                value: round_to(value, decimals),
                expected: None,
                deviation: None,
                score: None,
                lower: None,
                upper: None,
                severity: SEV_UNSCORED,
                anomaly: false,
                rule: "",
                rules_fired: Vec::new(),
                baseline_n: None,
            }
        } else {
            let worst = verdicts
                .iter()
                .copied()
                .max_by(|a, b| {
                    a.weight()
                        .partial_cmp(&b.weight())
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .unwrap();
            let fired: Vec<&'static str> = verdicts
                .iter()
                .filter(|v| sev_rank(v.severity) >= sev_rank(SEV_WARNING))
                .map(|v| v.rule)
                .collect();
            scored += 1;
            if worst.score.is_none() {
                flat_baseline += 1;
            }
            match worst.severity {
                SEV_CRITICAL => {}
                SEV_WARNING => warnings += 1,
                _ => normal += 1,
            }
            if let Some(s) = worst.score {
                let better = match max_score {
                    Some(m) => s.abs() > m,
                    None => true,
                };
                if better {
                    max_score = Some(s.abs());
                    max_score_index = Some(i + 1);
                }
            }
            let band = opts.threshold * worst.base.spread;
            let (lower, upper) = if worst.base.spread > 0.0 {
                (
                    Some(round_to(worst.base.center - band, decimals)),
                    Some(round_to(worst.base.center + band, decimals)),
                )
            } else {
                (None, None)
            };
            Point {
                index: i + 1,
                label: labels[i].clone(),
                value: round_to(value, decimals),
                expected: Some(round_to(worst.base.center, decimals)),
                deviation: Some(round_to(worst.deviation, decimals)),
                score: worst.score.map(|s| round_to(s, decimals)),
                lower,
                upper,
                severity: worst.severity,
                anomaly: worst.severity == SEV_CRITICAL,
                rule: worst.rule,
                rules_fired: fired,
                baseline_n: Some(worst.base.n),
            }
        };

        if row.anomaly {
            anomaly_indices.push(row.index);
            anomaly_values.push(row.value);
            anomaly_scores.push(row.score);
        }
        points.push(row);
    }

    let anomalies = anomaly_indices.len();
    let summary = Summary {
        points: n,
        scored,
        unscored: n - scored,
        anomalies,
        warnings,
        normal,
        anomaly_rate: if scored == 0 {
            0.0
        } else {
            round_to(anomalies as f64 / scored as f64 * 100.0, decimals.max(2))
        },
        flat_baseline,
        max_score: max_score.map(|s| round_to(s, decimals)),
        max_score_index,
        first_anomaly_index: anomaly_indices.first().copied(),
        last_anomaly_index: anomaly_indices.last().copied(),
        mean: round_to(mean_of(&values), decimals),
        std_dev: round_to(sd_of(&values), decimals),
    };

    let interpretation = interpret(method, &points, &summary, opts, window, period);

    if opts.only_anomalies {
        points.retain(|p| matches!(p.severity, SEV_CRITICAL | SEV_WARNING));
    }

    Ok(Report {
        count: n,
        method: method.name(),
        window: method.uses_rolling().then_some(window),
        min_periods: method.uses_rolling().then_some(min_periods),
        period,
        threshold: opts.threshold,
        warn_threshold: opts.warn_threshold,
        tolerance: opts.tolerance,
        direction: direction.name(),
        center: opts.center,
        points,
        anomaly_indices,
        anomaly_values,
        anomaly_scores,
        summary,
        interpretation,
    })
}

fn rule_phrase(method: Method, opts: &Options, window: usize, period: Option<usize>) -> String {
    let span = if opts.center { "centred" } else { "trailing" };
    match method {
        Method::RollingZ => format!("a {window}-point {span} rolling mean ± SD"),
        Method::RollingMad => format!("a {window}-point {span} rolling median ± MAD"),
        Method::SeasonalZ => format!(
            "the mean ± SD of the other points at the same position in a {}-point cycle",
            period.unwrap_or(0)
        ),
        Method::Combined => format!(
            "the worse of a {window}-point {span} rolling mean ± SD and a {}-point seasonal comparison",
            period.unwrap_or(0)
        ),
    }
}

fn interpret(
    method: Method,
    points: &[Point],
    summary: &Summary,
    opts: &Options,
    window: usize,
    period: Option<usize>,
) -> String {
    let rule = rule_phrase(method, opts, window, period);
    let mut s = format!(
        "Scored {} of {} points against {} — {} needed more history first. ",
        summary.scored, summary.points, rule, summary.unscored
    );
    if summary.anomalies == 0 {
        s.push_str(&format!(
            "Nothing crossed |score| ≥ {}",
            fmt_num(opts.threshold)
        ));
        if opts.direction != "both" {
            s.push_str(&format!(" {} the baseline", opts.direction));
        }
        s.push('.');
        if summary.warnings > 0 {
            s.push_str(&format!(
                " {} point(s) sit in the {}–{} watch band.",
                summary.warnings,
                fmt_num(opts.warn_threshold),
                fmt_num(opts.threshold)
            ));
        }
        if let (Some(m), Some(i)) = (summary.max_score, summary.max_score_index) {
            s.push_str(&format!(
                " The closest call is row {i} at |score| {}.",
                fmt_num(m)
            ));
        }
        return s;
    }
    s.push_str(&format!(
        "{} of {} scored points ({}%) crossed |score| ≥ {}",
        summary.anomalies,
        summary.scored,
        fmt_num(summary.anomaly_rate),
        fmt_num(opts.threshold)
    ));
    let worst = points
        .iter()
        .filter(|p| p.anomaly)
        .max_by(|a, b| {
            let (x, y) = (
                a.score.map(f64::abs).unwrap_or(f64::MAX),
                b.score.map(f64::abs).unwrap_or(f64::MAX),
            );
            x.partial_cmp(&y).unwrap_or(std::cmp::Ordering::Equal)
        })
        .unwrap();
    let where_ = if worst.label.is_empty() {
        format!("row {}", worst.index)
    } else {
        format!("row {} ({})", worst.index, worst.label)
    };
    match (worst.score, worst.expected) {
        (Some(sc), Some(exp)) => s.push_str(&format!(
            "; the strongest is {where_}: {} against an expected {} (score {}).",
            fmt_num(worst.value),
            fmt_num(exp),
            fmt_num(sc)
        )),
        (None, Some(exp)) => s.push_str(&format!(
            "; the strongest is {where_}: {} against a perfectly flat baseline of {} (no finite score).",
            fmt_num(worst.value),
            fmt_num(exp)
        )),
        _ => s.push('.'),
    }
    if summary.warnings > 0 {
        s.push_str(&format!(
            " Another {} point(s) sit in the {}–{} watch band.",
            summary.warnings,
            fmt_num(opts.warn_threshold),
            fmt_num(opts.threshold)
        ));
    }
    if summary.flat_baseline > 0 {
        s.push_str(&format!(
            " {} point(s) had a flat baseline, so their verdict rests on the tolerance ({}) rather than a score.",
            summary.flat_baseline,
            fmt_num(opts.tolerance)
        ));
    }
    s
}

/// Compact number formatting for the table/CSV/prose surfaces.
pub fn fmt_num(v: f64) -> String {
    if !v.is_finite() {
        return "n/a".into();
    }
    if v == 0.0 {
        return "0".into();
    }
    if v.fract() == 0.0 && v.abs() < 1e15 {
        return format!("{}", v as i64);
    }
    let mag = v.abs();
    let decimals = if mag >= 100.0 {
        2
    } else if mag >= 1.0 {
        4
    } else {
        6
    };
    let s = format!("{v:.decimals$}");
    let s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    if s == "-0" {
        "0".into()
    } else {
        s
    }
}

fn opt_num(v: Option<f64>) -> String {
    v.map(fmt_num).unwrap_or_else(|| "-".into())
}

// ---------------------------------------------------------------------------
// rendering
// ---------------------------------------------------------------------------

fn render_table(r: &Report) -> String {
    let has_labels = r.points.iter().any(|p| !p.label.is_empty());
    let mut head: Vec<String> = vec!["#".into()];
    if has_labels {
        head.push("Label".into());
    }
    head.extend(
        [
            "Value",
            "Expected",
            "Deviation",
            "Score",
            "Band",
            "Severity",
        ]
        .iter()
        .map(|s| s.to_string()),
    );
    if r.method == "combined" {
        head.push("Rule".into());
    }

    let mut rows: Vec<Vec<String>> = Vec::with_capacity(r.points.len());
    for p in &r.points {
        let mut row: Vec<String> = vec![p.index.to_string()];
        if has_labels {
            row.push(p.label.clone());
        }
        let band = match (p.lower, p.upper) {
            (Some(l), Some(u)) => format!("{} … {}", fmt_num(l), fmt_num(u)),
            _ => "-".into(),
        };
        row.push(fmt_num(p.value));
        row.push(opt_num(p.expected));
        row.push(opt_num(p.deviation));
        row.push(opt_num(p.score));
        row.push(band);
        row.push(p.severity.to_string());
        if r.method == "combined" {
            row.push(if p.rule.is_empty() {
                "-".into()
            } else {
                p.rule.to_string()
            });
        }
        rows.push(row);
    }

    let mut widths: Vec<usize> = head.iter().map(|h| h.chars().count()).collect();
    for row in &rows {
        for (c, cell) in row.iter().enumerate() {
            widths[c] = widths[c].max(cell.chars().count());
        }
    }
    let line = |cells: &[String]| -> String {
        cells
            .iter()
            .enumerate()
            .map(|(c, cell)| format!("{:<width$}", cell, width = widths[c]))
            .collect::<Vec<_>>()
            .join("  ")
            .trim_end()
            .to_string()
    };

    let mut out = String::new();
    let mut header = format!("{} · n {}", r.method, r.count);
    if let Some(w) = r.window {
        header.push_str(&format!(
            " · window {w}{}",
            if r.center {
                " (centred)"
            } else {
                " (trailing)"
            }
        ));
    }
    if let Some(p) = r.period {
        header.push_str(&format!(" · period {p}"));
    }
    header.push_str(&format!(
        " · threshold {} · warn {} · tolerance {} · direction {}",
        fmt_num(r.threshold),
        fmt_num(r.warn_threshold),
        fmt_num(r.tolerance),
        r.direction
    ));
    out.push_str(&header);
    out.push('\n');
    out.push_str(&format!(
        "anomalies {} · warnings {} · scored {} · unscored {} · anomaly rate {}%\n\n",
        r.summary.anomalies,
        r.summary.warnings,
        r.summary.scored,
        r.summary.unscored,
        fmt_num(r.summary.anomaly_rate)
    ));
    out.push_str(&line(&head));
    out.push('\n');
    let dashes: Vec<String> = widths.iter().map(|w| "-".repeat(*w)).collect();
    out.push_str(&line(&dashes));
    out.push('\n');
    if rows.is_empty() {
        out.push_str("(no warning or critical rows)\n");
    }
    for row in &rows {
        out.push_str(&line(row));
        out.push('\n');
    }
    out.push('\n');
    out.push_str(&r.interpretation);
    out.push('\n');
    out
}

fn csv_cell(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn render_csv(r: &Report) -> String {
    let mut out = String::from(
        "index,label,value,expected,deviation,score,lower,upper,severity,anomaly,rule\n",
    );
    for p in &r.points {
        out.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{}\n",
            p.index,
            csv_cell(&p.label),
            fmt_num(p.value),
            opt_num(p.expected),
            opt_num(p.deviation),
            opt_num(p.score),
            opt_num(p.lower),
            opt_num(p.upper),
            p.severity,
            p.anomaly,
            p.rule
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

    fn opts() -> Options {
        Options::default()
    }

    /// A flat history with one spike: the spike must be the only anomaly, and its
    /// own value must not contaminate the baseline it is compared against.
    #[test]
    fn rolling_z_flags_a_spike_against_its_recent_history() {
        let mut o = opts();
        o.window = 5;
        o.min_periods = 3;
        let r = analyze("10 11 9 10 12 10 11 40 10 11", &o).unwrap();
        assert_eq!(r.summary.anomalies, 1);
        assert_eq!(r.anomaly_indices, vec![8]);
        assert_eq!(r.anomaly_values, vec![40.0]);
        let spike = &r.points[7];
        assert_eq!(spike.severity, "critical");
        assert!(spike.anomaly);
        assert_eq!(spike.rule, "rolling");
        // Baseline = rows 3..7 (9, 10, 12, 10, 11) → mean 10.4, so the spike's own
        // 40 never entered the expected value.
        assert_eq!(spike.expected, Some(10.4));
        assert!(spike.score.unwrap() > 20.0, "score {:?}", spike.score);
        // The first two rows have too little history to judge.
        assert_eq!(r.points[0].severity, "unscored");
        assert_eq!(r.points[0].score, None);
        assert_eq!(r.summary.unscored, 3);
    }

    #[test]
    fn warn_band_labels_near_misses_without_flagging_them() {
        let mut o = opts();
        o.window = 6;
        o.min_periods = 3;
        o.threshold = 30.0;
        o.warn_threshold = 3.0;
        let r = analyze("10 11 9 10 12 10 11 40 10 11", &o).unwrap();
        assert_eq!(r.summary.anomalies, 0, "40 is under the raised threshold");
        assert_eq!(r.summary.warnings, 1);
        assert_eq!(r.points[7].severity, "warning");
        assert!(!r.points[7].anomaly);
        assert!(r.anomaly_indices.is_empty());
    }

    /// The classic z-score weakness: a spike inside the window inflates the SD and
    /// masks the next one. The MAD arm should not care.
    #[test]
    fn rolling_mad_is_robust_where_rolling_z_is_masked() {
        let series = "10 11 10 9 10 11 60 10 9 11 10 10 55";
        let mut z = opts();
        z.window = 6;
        z.min_periods = 3;
        z.method = "rolling_z".into();
        let rz = analyze(series, &z).unwrap();

        let mut mad = z.clone();
        mad.method = "rolling_mad".into();
        let rm = analyze(series, &mad).unwrap();

        assert!(
            rm.summary.anomalies > rz.summary.anomalies,
            "mad {} should beat z {}",
            rm.summary.anomalies,
            rz.summary.anomalies
        );
        assert!(rm.anomaly_indices.contains(&13), "{:?}", rm.anomaly_indices);
        assert!(
            !rz.anomaly_indices.contains(&13),
            "{:?}",
            rz.anomaly_indices
        );
        // The MAD baseline shrugs off the 60 sitting in its own window: median 10,
        // so row 13 still gets a big FINITE score instead of being masked.
        assert_eq!(rm.points[12].expected, Some(10.0));
        assert!(rm.points[12].score.unwrap() > 10.0, "{:?}", rm.points[12]);
    }

    /// A weekly pattern where Sunday is always low: the rolling rule calls the
    /// normal Sundays anomalies, the seasonal rule does not — but it does catch a
    /// Sunday that broke its own pattern.
    #[test]
    fn seasonal_rule_judges_each_point_against_its_own_phase() {
        // period 3: phase 0 ≈ 100, phase 1 ≈ 200, phase 2 ≈ 300, with row 13
        // (phase 0) spiking to 180.
        let series = "100 200 300 101 201 301 99 199 299 100 200 300 180 200 300";
        let mut o = opts();
        o.method = "seasonal_z".into();
        o.period = 3;
        let r = analyze(series, &o).unwrap();
        assert_eq!(r.period, Some(3));
        assert_eq!(r.window, None, "window is irrelevant to the seasonal rule");
        assert_eq!(r.anomaly_indices, vec![13], "{:?}", r.points[12]);
        assert_eq!(r.points[12].rule, "seasonal");
        assert_eq!(r.points[12].expected, Some(100.0));
        // The first cycle has no same-phase peers yet.
        assert_eq!(r.points[0].severity, "unscored");
    }

    #[test]
    fn combined_reports_which_rule_fired() {
        let series = "100 200 300 101 201 301 99 199 299 100 200 300 180 200 300";
        let mut o = opts();
        o.method = "combined".into();
        o.period = 3;
        o.window = 4;
        o.min_periods = 3;
        let r = analyze(series, &o).unwrap();
        assert_eq!(r.window, Some(4));
        assert_eq!(r.period, Some(3));
        let row = &r.points[12];
        assert!(row.anomaly);
        assert!(
            row.rules_fired.contains(&"seasonal"),
            "{:?}",
            row.rules_fired
        );
    }

    #[test]
    fn direction_filters_one_sided_alerts() {
        let mut o = opts();
        o.window = 5;
        o.min_periods = 3;
        o.direction = "below".into();
        let r = analyze("10 11 9 10 12 10 11 40 10 11", &o).unwrap();
        assert_eq!(r.summary.anomalies, 0, "the spike is above, not below");
        assert_eq!(r.points[7].severity, "normal");

        o.direction = "above".into();
        let up = analyze("10 11 9 10 12 10 11 40 10 11", &o).unwrap();
        assert_eq!(up.anomaly_indices, vec![8]);
    }

    #[test]
    fn tolerance_deadband_suppresses_tiny_deviations() {
        // A near-flat series: the 0.3 step is a huge z but a trivial move.
        let series = "5 5 5.1 5 5 5.1 5 5.3 5 5";
        let mut o = opts();
        o.window = 5;
        o.min_periods = 3;
        let loud = analyze(series, &o).unwrap();
        assert!(loud.summary.anomalies >= 1);

        o.tolerance = 0.5;
        let quiet = analyze(series, &o).unwrap();
        assert_eq!(quiet.summary.anomalies, 0);
        assert_eq!(quiet.summary.warnings, 0);
        assert_eq!(quiet.points[7].severity, "normal");
    }

    #[test]
    fn a_flat_baseline_reports_a_break_without_a_score() {
        let mut o = opts();
        o.window = 4;
        o.min_periods = 3;
        let r = analyze("7 7 7 7 7 7 9 7", &o).unwrap();
        let brk = &r.points[6];
        assert_eq!(brk.severity, "critical");
        assert_eq!(brk.score, None, "no finite score against a flat baseline");
        assert_eq!(brk.expected, Some(7.0));
        assert_eq!(brk.lower, None);
        // Every scored row here sits on a flat window; only the 9 clears the
        // tolerance, so only it is flagged.
        assert_eq!(r.summary.flat_baseline, 4);
        assert_eq!(r.summary.anomalies, 1);
        assert_eq!(r.anomaly_scores, vec![None]);
        assert!(r.interpretation.contains("flat baseline"));
    }

    #[test]
    fn centered_window_scores_the_very_first_points() {
        let mut o = opts();
        o.window = 4;
        o.min_periods = 2;
        o.center = true;
        let r = analyze("10 10 10 40 10 10 10 10", &o).unwrap();
        assert_eq!(r.points[0].severity, "normal", "centred: row 1 has peers");
        assert_eq!(r.summary.unscored, 0);
        assert_eq!(r.anomaly_indices, vec![4]);
    }

    #[test]
    fn labels_and_headers_survive_the_parser() {
        let mut o = opts();
        o.window = 3;
        o.min_periods = 2;
        let r = analyze(
            "date,visits\n2026-01-01, 100\n2026-01-02, 104\n2026-01-03, 98\n2026-01-04, 400\n",
            &o,
        )
        .unwrap();
        assert_eq!(r.count, 4, "the header row is skipped, not parsed");
        assert_eq!(r.points[3].label, "2026-01-04");
        assert_eq!(r.anomaly_indices, vec![4]);
        assert!(r.interpretation.contains("2026-01-04"));
    }

    #[test]
    fn only_anomalies_keeps_the_flagged_rows_but_not_the_counts() {
        let mut o = opts();
        o.window = 5;
        o.min_periods = 3;
        o.only_anomalies = true;
        let r = analyze("10 11 9 10 12 10 11 40 10 11", &o).unwrap();
        // Row 5 is a warning (|z| 2.45), row 8 the anomaly — both are kept.
        assert_eq!(r.points.len(), 2);
        assert_eq!(r.points[0].index, 5);
        assert_eq!(r.points[0].severity, "warning");
        assert_eq!(r.points[1].index, 8);
        assert_eq!(r.points[1].severity, "critical");
        assert_eq!(r.summary.points, 10, "the summary still counts every row");
        assert_eq!(r.summary.scored, 7);
    }

    #[test]
    fn table_and_csv_render_the_same_verdicts() {
        let mut o = opts();
        o.window = 5;
        o.min_periods = 3;
        o.output = "table".into();
        let table = render("10 11 9 10 12 10 11 40 10 11", &o).unwrap();
        assert!(table.contains("rolling_z · n 10 · window 5 (trailing)"));
        assert!(table.contains("critical"));
        assert!(table
            .lines()
            .any(|l| l.starts_with("8  ") && l.contains("40")));

        o.output = "csv".into();
        let csv = render("10 11 9 10 12 10 11 40 10 11", &o).unwrap();
        let mut lines = csv.lines();
        assert_eq!(
            lines.next().unwrap(),
            "index,label,value,expected,deviation,score,lower,upper,severity,anomaly,rule"
        );
        assert_eq!(lines.clone().count(), 10);
        let row8 = csv.lines().nth(8).unwrap();
        assert!(row8.starts_with("8,,40,10.4,29.6,"), "{row8}");
        assert!(row8.ends_with(",critical,true,rolling"), "{row8}");
    }

    #[test]
    fn decimals_round_every_emitted_number() {
        let mut o = opts();
        o.window = 4;
        o.min_periods = 3;
        o.decimals = 2;
        let r = analyze("1 2 3 4 5 6 7 8 9 30", &o).unwrap();
        let last = r.points.last().unwrap();
        assert_eq!(last.score, Some(round_to(last.score.unwrap(), 2)));
        assert_eq!(last.expected, Some(7.5));
    }

    #[test]
    fn json_output_is_the_default_and_carries_the_anomaly_arrays() {
        let mut o = opts();
        o.window = 5;
        o.min_periods = 3;
        let json = render("10 11 9 10 12 10 11 40 10 11", &o).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["method"], "rolling_z");
        assert_eq!(v["anomaly_indices"][0], 8);
        assert_eq!(v["anomaly_values"][0], 40.0);
        assert_eq!(v["summary"]["anomalies"], 1);
        assert!(v["period"].is_null(), "no seasonal rule ran");
        assert_eq!(v["points"][7]["severity"], "critical");
    }

    // ---- error paths -----------------------------------------------------

    #[test]
    fn too_few_values_is_an_error() {
        let e = analyze("10, 12", &opts()).unwrap_err();
        assert!(e.contains("at least 3 numeric values"), "{e}");
    }

    #[test]
    fn a_non_numeric_row_names_the_line() {
        let e = analyze("10\n12\nn/a\n14", &opts()).unwrap_err();
        assert_eq!(e, "line 3: 'n/a' is not a number");
    }

    #[test]
    fn unknown_method_lists_the_choices() {
        let mut o = opts();
        o.method = "prophet".into();
        let e = analyze("1 2 3", &o).unwrap_err();
        assert!(
            e.contains("rolling_z, rolling_mad, seasonal_z, combined"),
            "{e}"
        );
    }

    #[test]
    fn a_seasonal_run_needs_three_cycles() {
        let mut o = opts();
        o.method = "seasonal_z".into();
        o.period = 7;
        let e = analyze("1 2 3 4 5 6 7 8 9 10", &o).unwrap_err();
        assert!(e.contains("period 7 needs 15 values, got 10"), "{e}");
        assert!(
            e.contains("rolling_z"),
            "the error suggests a way forward: {e}"
        );
    }

    #[test]
    fn warn_threshold_above_threshold_is_rejected() {
        let mut o = opts();
        o.warn_threshold = 4.0;
        let e = analyze("1 2 3 4 5", &o).unwrap_err();
        assert!(e.contains("must not exceed threshold"), "{e}");
    }

    #[test]
    fn min_periods_above_window_is_rejected() {
        let mut o = opts();
        o.window = 5;
        o.min_periods = 9;
        let e = analyze("1 2 3 4 5 6", &o).unwrap_err();
        assert!(e.contains("cannot exceed window"), "{e}");
    }

    #[test]
    fn bad_window_and_output_are_rejected() {
        let mut o = opts();
        o.window = 1;
        assert!(analyze("1 2 3", &o).unwrap_err().contains("window must be"));
        let mut o2 = opts();
        o2.output = "svg".into();
        assert!(render("1 2 3", &o2)
            .unwrap_err()
            .contains("json, table, csv"));
    }

    #[test]
    fn a_negative_tolerance_is_rejected() {
        let mut o = opts();
        o.tolerance = -1.0;
        let e = analyze("1 2 3 4", &o).unwrap_err();
        assert!(e.contains("tolerance must be 0 or more"), "{e}");
    }

    #[test]
    fn too_many_values_is_capped() {
        let big = (0..MAX_POINTS + 1)
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let e = analyze(&big, &opts()).unwrap_err();
        assert!(e.contains("exceeds the 20000 limit"), "{e}");
    }
}
