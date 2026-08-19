//! Seasonal-trend decomposition of an evenly spaced time series, pure and deterministic.
//!
//! Splits a pasted series into trend, seasonal, and residual components with either
//! STL (seasonal-trend decomposition by loess) or the classical moving-average
//! method, under an additive or multiplicative model, and renders a four-panel
//! standalone SVG — or a components table / CSV / JSON block.
//!
//! No plotting library, no I/O, no external data: the same code runs in the chat
//! Service Worker, the CLI, and the browser page.
#![forbid(unsafe_code)]

use std::fmt::Write as _;

/// Hard caps so a paste-bomb can't hang the browser tab.
pub const MAX_POINTS: usize = 10_000;
/// Two full cycles of the smallest possible period (2).
pub const MIN_POINTS: usize = 4;
pub const MAX_PERIOD: usize = 1_000;
/// Longest lag considered by the automatic period search.
const MAX_AUTO_LAG: usize = 400;
/// Minimum autocorrelation a candidate lag needs before it is trusted.
const AUTO_ACF_FLOOR: f64 = 0.2;

const FONT: &str = "system-ui, -apple-system, 'Segoe UI', Roboto, Helvetica, Arial, sans-serif";

/// Every knob the decomposer accepts. Mirrors the block descriptor 1:1.
#[derive(Debug, Clone)]
pub struct Options {
    pub method: String,
    pub model: String,
    pub period: u32,
    pub seasonal_window: u32,
    pub trend_window: u32,
    pub robust: bool,
    pub two_sided: bool,
    pub extrapolate_trend: bool,
    pub trend_overlay: bool,
    pub show_adjusted: bool,
    pub residual_style: String,
    pub grid: bool,
    pub title: String,
    pub x_label: String,
    pub y_label: String,
    pub width: u32,
    pub height: u32,
    pub color: String,
    pub theme: String,
    pub precision: u32,
    pub output: String,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            method: "stl".into(),
            model: "additive".into(),
            period: 0,
            seasonal_window: 0,
            trend_window: 0,
            robust: false,
            two_sided: true,
            extrapolate_trend: true,
            trend_overlay: true,
            show_adjusted: false,
            residual_style: "bar".into(),
            grid: true,
            title: String::new(),
            x_label: String::new(),
            y_label: String::new(),
            width: 900,
            height: 720,
            color: "#2563eb".into(),
            theme: "light".into(),
            precision: 4,
            output: "svg".into(),
        }
    }
}

/// The decomposed series plus everything the renderers report.
#[derive(Debug, Clone)]
pub struct Decomposition {
    /// Row labels as pasted (empty strings when the input was values only).
    pub labels: Vec<String>,
    pub observed: Vec<f64>,
    pub trend: Vec<f64>,
    pub seasonal: Vec<f64>,
    pub residual: Vec<f64>,
    /// Observed with the seasonal component removed.
    pub adjusted: Vec<f64>,
    /// Average seasonal effect at each position in the cycle.
    pub seasonal_indices: Vec<f64>,
    pub period: usize,
    /// `"auto"` when the period was detected, `"explicit"` when it was given.
    pub period_source: String,
    /// Resolved method, never blank: `stl` or `classical`.
    pub method: String,
    pub model: String,
    pub robust: bool,
    /// `"periodic"` or the odd seasonal-smoother length actually used (STL only).
    pub seasonal_window: String,
    /// Trend-smoother length (STL) or moving-average order (classical).
    pub trend_window: usize,
    pub n: usize,
    /// fpp-style strength of trend, 0-1, on the additive (log for multiplicative) scale.
    pub trend_strength: f64,
    /// fpp-style strength of seasonality, 0-1, same scale.
    pub seasonal_strength: f64,
}

// ---------------------------------------------------------------------------
// entry points
// ---------------------------------------------------------------------------

/// Parse, decompose, and render in the requested `output` format.
pub fn render(data: &str, opts: &Options) -> Result<String, String> {
    let d = analyze(data, opts)?;
    match normalize_opt(&opts.output, "svg") {
        "svg" => Ok(render_svg(&d, opts)),
        "table" => Ok(render_table(&d, opts)),
        "csv" => Ok(render_csv(&d, opts)),
        "json" => Ok(render_json(&d, opts)),
        other => Err(format!(
            "unknown output '{other}': expected one of svg, table, csv, json"
        )),
    }
}

/// Parse + decompose without rendering. Useful for tests and other callers.
pub fn analyze(data: &str, opts: &Options) -> Result<Decomposition, String> {
    let (values, labels) = parse_series(data)?;
    let n = values.len();

    let method = match normalize_opt(&opts.method, "stl") {
        "stl" => "stl",
        "classical" => "classical",
        other => {
            return Err(format!(
                "unknown method '{other}': expected stl or classical"
            ))
        }
    };
    let model = match normalize_opt(&opts.model, "additive") {
        "additive" => "additive",
        "multiplicative" => "multiplicative",
        other => {
            return Err(format!(
                "unknown model '{other}': expected additive or multiplicative"
            ))
        }
    };

    // Resolve the seasonal period.
    let (period, period_source) = if opts.period == 0 {
        match detect_period(&values) {
            Some(p) => (p, "auto"),
            None => return Err(
                "could not detect a seasonal period automatically: set period explicitly \
                 (12 for monthly data with a yearly cycle, 4 for quarterly, 7 for daily data \
                 with a weekly cycle, 24 for hourly data with a daily cycle)"
                    .into(),
            ),
        }
    } else {
        (opts.period as usize, "explicit")
    };
    if period < 2 {
        return Err(format!(
            "period must be at least 2, got {period}: a period of 1 has no seasonal cycle"
        ));
    }
    if period > MAX_PERIOD {
        return Err(format!("period must be at most {MAX_PERIOD}, got {period}"));
    }
    if n < 2 * period {
        return Err(format!(
            "decomposition needs at least 2 full cycles: period {period} requires {} values, got {n}",
            2 * period
        ));
    }

    // Multiplicative decomposes in log space, so every value has to be positive.
    let work: Vec<f64> = if model == "multiplicative" {
        if let Some(i) = values.iter().position(|v| *v <= 0.0) {
            return Err(format!(
                "multiplicative model needs every value to be greater than 0, but value {} at position {} is not: use the additive model instead",
                fmt_num(values[i]),
                i + 1
            ));
        }
        values.iter().map(|v| v.ln()).collect()
    } else {
        values.clone()
    };

    let fit = if method == "stl" {
        let (sw_used, sw_label) = resolve_seasonal_window(opts.seasonal_window);
        let nt = resolve_trend_window(opts.trend_window, period, sw_used);
        let (trend, seasonal) = stl(&work, period, sw_used, nt, opts.robust);
        Fit {
            trend,
            seasonal,
            seasonal_window: sw_label,
            trend_window: nt,
        }
    } else {
        let (trend, seasonal) = classical(&work, period, opts.two_sided, opts.extrapolate_trend);
        Fit {
            trend,
            seasonal,
            seasonal_window: "n/a".into(),
            trend_window: period,
        }
    };

    let residual: Vec<f64> = (0..n)
        .map(|i| work[i] - fit.trend[i] - fit.seasonal[i])
        .collect();
    let (trend_strength, seasonal_strength) =
        strengths(&fit.trend, &fit.seasonal, &residual);

    // Seasonal index per position in the cycle, averaged over cycles.
    let mut seasonal_indices = vec![0.0; period];
    for k in 0..period {
        let mut sum = 0.0;
        let mut cnt = 0usize;
        let mut i = k;
        while i < n {
            if fit.seasonal[i].is_finite() {
                sum += fit.seasonal[i];
                cnt += 1;
            }
            i += period;
        }
        seasonal_indices[k] = if cnt > 0 {
            sum / cnt as f64
        } else {
            f64::NAN
        };
    }

    // Back to the observed scale for the multiplicative model.
    let (trend, seasonal, residual, seasonal_indices) = if model == "multiplicative" {
        (
            fit.trend.iter().map(|v| v.exp()).collect::<Vec<f64>>(),
            fit.seasonal.iter().map(|v| v.exp()).collect::<Vec<f64>>(),
            residual.iter().map(|v| v.exp()).collect::<Vec<f64>>(),
            seasonal_indices.iter().map(|v| v.exp()).collect::<Vec<f64>>(),
        )
    } else {
        (fit.trend, fit.seasonal, residual, seasonal_indices)
    };

    let adjusted: Vec<f64> = (0..n)
        .map(|i| {
            if model == "multiplicative" {
                values[i] / seasonal[i]
            } else {
                values[i] - seasonal[i]
            }
        })
        .collect();

    Ok(Decomposition {
        labels,
        observed: values,
        trend,
        seasonal,
        residual,
        adjusted,
        seasonal_indices,
        period,
        period_source: period_source.into(),
        method: method.into(),
        model: model.into(),
        robust: opts.robust && method == "stl",
        seasonal_window: fit.seasonal_window,
        trend_window: fit.trend_window,
        n,
        trend_strength,
        seasonal_strength,
    })
}

struct Fit {
    trend: Vec<f64>,
    seasonal: Vec<f64>,
    seasonal_window: String,
    trend_window: usize,
}

// ---------------------------------------------------------------------------
// input parsing
// ---------------------------------------------------------------------------

fn normalize_opt<'a>(s: &'a str, fallback: &'a str) -> &'a str {
    let t = s.trim();
    if t.is_empty() {
        fallback
    } else {
        t
    }
}

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

/// Accepts one value per line, `label,value` rows, or a single separated line of
/// values. A leading all-text header row is skipped.
fn parse_series(data: &str) -> Result<(Vec<f64>, Vec<String>), String> {
    let lines: Vec<(usize, &str)> = data
        .lines()
        .enumerate()
        .map(|(i, l)| (i + 1, l.trim_end_matches('\r').trim()))
        .filter(|(_, l)| !l.is_empty())
        .collect();
    if lines.is_empty() {
        return Err(format!(
            "no data found: paste at least {MIN_POINTS} numbers, one per line (or `label,value` rows)"
        ));
    }

    let mut values: Vec<f64> = Vec::new();
    let mut labels: Vec<String> = Vec::new();

    if lines.len() == 1 {
        // A single line of separated values.
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
                    None => {
                        return Err(format!("line {lineno}: '{}' is not a number", toks[0]));
                    }
                }
            } else {
                // `label, value` — the label may itself contain separators, so the
                // value is the last numeric token and the label is what precedes it.
                let value_at = toks.iter().rposition(|t| parse_num(t).is_some());
                match value_at {
                    Some(idx) if idx > 0 => {
                        values.push(parse_num(toks[idx]).unwrap());
                        labels.push(toks[..idx].join(" "));
                    }
                    Some(idx) => {
                        // First token is the number: treat the rest as a trailing label.
                        values.push(parse_num(toks[idx]).unwrap());
                        labels.push(toks[1..].join(" "));
                    }
                    None if values.is_empty() => continue, // header row
                    None => {
                        return Err(format!(
                            "line {lineno}: no number found in '{}'",
                            toks.join(" ")
                        ));
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
    if labels.iter().all(|l| l.is_empty()) {
        labels.clear();
        labels.resize(values.len(), String::new());
    }
    Ok((values, labels))
}

// ---------------------------------------------------------------------------
// automatic period detection
// ---------------------------------------------------------------------------

/// Strongest autocorrelation peak of the linearly detrended series, preferring the
/// base period over its harmonics. `None` when nothing is convincing enough.
pub fn detect_period(y: &[f64]) -> Option<usize> {
    let n = y.len();
    if n < 2 * 2 {
        return None;
    }
    // Remove a least-squares line so the trend doesn't dominate every lag.
    let nf = n as f64;
    let xbar = (nf - 1.0) / 2.0;
    let ybar = y.iter().sum::<f64>() / nf;
    let mut sxy = 0.0;
    let mut sxx = 0.0;
    for (i, v) in y.iter().enumerate() {
        let dx = i as f64 - xbar;
        sxy += dx * (v - ybar);
        sxx += dx * dx;
    }
    let slope = if sxx > 0.0 { sxy / sxx } else { 0.0 };
    let d: Vec<f64> = y
        .iter()
        .enumerate()
        .map(|(i, v)| v - (ybar + slope * (i as f64 - xbar)))
        .collect();

    let denom: f64 = d.iter().map(|v| v * v).sum();
    if !(denom > 0.0) {
        return None;
    }
    // A perfectly straight line leaves only floating-point dust behind, which has
    // its own spurious autocorrelation peaks — treat it as "no cycle here".
    let scale = y.iter().map(|v| v.abs()).fold(0.0_f64, f64::max).max(1.0);
    if denom / nf <= 1e-18 * scale * scale {
        return None;
    }
    let max_lag = (n / 2).min(MAX_AUTO_LAG);
    if max_lag < 3 {
        return None;
    }
    let mut acf = vec![0.0; max_lag + 1];
    for (lag, slot) in acf.iter_mut().enumerate().skip(1) {
        let mut s = 0.0;
        for i in lag..n {
            s += d[i] * d[i - lag];
        }
        *slot = s / denom;
    }

    let mut best: Option<usize> = None;
    let mut best_v = AUTO_ACF_FLOOR;
    for lag in 2..max_lag {
        if acf[lag] > acf[lag - 1] && acf[lag] >= acf[lag + 1] && acf[lag] > best_v {
            best_v = acf[lag];
            best = Some(lag);
        }
    }
    let lag = best?;
    // A harmonic can edge out the base period; prefer the smallest divisor whose
    // autocorrelation is nearly as strong.
    let mut chosen = lag;
    for cand in 2..lag {
        if lag % cand == 0 && acf[cand] >= 0.85 * acf[lag] && n >= 2 * cand {
            chosen = cand;
            break;
        }
    }
    if n >= 2 * chosen {
        Some(chosen)
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// classical decomposition
// ---------------------------------------------------------------------------

fn centered_ma(y: &[f64], p: usize) -> Vec<f64> {
    let n = y.len();
    let mut t = vec![f64::NAN; n];
    if p < 2 || p > n {
        return t;
    }
    let h = p / 2;
    if p % 2 == 1 {
        for i in h..n - h {
            t[i] = y[i - h..=i + h].iter().sum::<f64>() / p as f64;
        }
    } else {
        for i in h..n - h {
            let mut s = 0.5 * y[i - h] + 0.5 * y[i + h];
            for v in y.iter().take(i + h).skip(i - h + 1) {
                s += v;
            }
            t[i] = s / p as f64;
        }
    }
    t
}

fn trailing_ma(y: &[f64], p: usize) -> Vec<f64> {
    let n = y.len();
    let mut t = vec![f64::NAN; n];
    if p < 2 || p > n {
        return t;
    }
    for i in p - 1..n {
        t[i] = y[i + 1 - p..=i].iter().sum::<f64>() / p as f64;
    }
    t
}

/// Least-squares extension of the trend across the leading/trailing gaps, fitted on
/// the `npoints` valid values closest to each gap.
fn extrapolate_ends(t: &mut [f64], npoints: usize) {
    let n = t.len();
    let first = match t.iter().position(|v| v.is_finite()) {
        Some(i) => i,
        None => return,
    };
    let last = t.iter().rposition(|v| v.is_finite()).unwrap_or(first);
    fn fit(t: &[f64], lo: usize, hi: usize) -> (f64, f64) {
        let pts: Vec<(f64, f64)> = (lo..=hi)
            .filter(|i| t[*i].is_finite())
            .map(|i| (i as f64, t[i]))
            .collect();
        if pts.len() < 2 {
            return (t[lo.min(t.len() - 1)], 0.0);
        }
        let m = pts.len() as f64;
        let xbar = pts.iter().map(|p| p.0).sum::<f64>() / m;
        let ybar = pts.iter().map(|p| p.1).sum::<f64>() / m;
        let mut sxy = 0.0;
        let mut sxx = 0.0;
        for (x, y) in &pts {
            sxy += (x - xbar) * (y - ybar);
            sxx += (x - xbar) * (x - xbar);
        }
        let b = if sxx > 0.0 { sxy / sxx } else { 0.0 };
        (ybar - b * xbar, b)
    }
    if first > 0 {
        let hi = (first + npoints.max(2) - 1).min(last);
        let (a, b) = fit(t, first, hi);
        for (i, slot) in t.iter_mut().enumerate().take(first) {
            *slot = a + b * i as f64;
        }
    }
    if last + 1 < n {
        let lo = last.saturating_sub(npoints.max(2) - 1).max(first);
        let (a, b) = fit(t, lo, last);
        for (i, slot) in t.iter_mut().enumerate().skip(last + 1) {
            *slot = a + b * i as f64;
        }
    }
}

/// Classical moving-average decomposition on the additive (or log) scale.
fn classical(y: &[f64], period: usize, two_sided: bool, extrapolate: bool) -> (Vec<f64>, Vec<f64>) {
    let n = y.len();
    let mut trend = if two_sided {
        centered_ma(y, period)
    } else {
        trailing_ma(y, period)
    };
    if extrapolate {
        extrapolate_ends(&mut trend, period);
    }

    // Average the detrended values at each position in the cycle, then centre them.
    let mut idx = vec![0.0; period];
    for (k, slot) in idx.iter_mut().enumerate() {
        let mut sum = 0.0;
        let mut cnt = 0usize;
        let mut i = k;
        while i < n {
            let d = y[i] - trend[i];
            if d.is_finite() {
                sum += d;
                cnt += 1;
            }
            i += period;
        }
        *slot = if cnt > 0 { sum / cnt as f64 } else { 0.0 };
    }
    let mean = idx.iter().sum::<f64>() / period as f64;
    for v in idx.iter_mut() {
        *v -= mean;
    }
    let seasonal: Vec<f64> = (0..n).map(|i| idx[i % period]).collect();
    (trend, seasonal)
}

// ---------------------------------------------------------------------------
// STL (seasonal-trend decomposition by loess)
// ---------------------------------------------------------------------------

fn next_odd(v: f64) -> usize {
    let mut k = v.ceil().max(3.0) as usize;
    if k % 2 == 0 {
        k += 1;
    }
    k
}

/// `0` selects the classic `periodic` seasonal smoother (one fixed shape per cycle).
fn resolve_seasonal_window(raw: u32) -> (usize, String) {
    if raw == 0 {
        (0, "periodic".into())
    } else {
        let mut k = raw.max(3) as usize;
        if k % 2 == 0 {
            k += 1;
        }
        (k, k.to_string())
    }
}

fn resolve_trend_window(raw: u32, period: usize, seasonal_window: usize) -> usize {
    if raw > 0 {
        let mut k = raw.max(3) as usize;
        if k % 2 == 0 {
            k += 1;
        }
        return k;
    }
    // Standard rule: nextodd(ceil(1.5 * period / (1 - 1.5 / n_s))), with the
    // `periodic` smoother behaving like an effectively infinite window.
    let ns = if seasonal_window == 0 {
        f64::INFINITY
    } else {
        seasonal_window as f64
    };
    let denom = 1.0 - 1.5 / ns;
    next_odd(1.5 * period as f64 / denom.max(1e-6))
}

/// Local linear (or constant) regression estimate at `x`, using the `q` nearest
/// points and tricube distance weights times the robustness weights.
fn loess_at(y: &[f64], rw: &[f64], q: usize, degree: usize, x: f64) -> f64 {
    let n = y.len();
    if n == 0 {
        return f64::NAN;
    }
    if n == 1 {
        return y[0];
    }
    let q_eff = q.max(2).min(n);
    let half = (q_eff as f64 - 1.0) / 2.0;
    let max_lo = (n - q_eff) as isize;
    let mut lo = (x - half).round() as isize;
    if lo < 0 {
        lo = 0;
    }
    if lo > max_lo {
        lo = max_lo;
    }
    let lo = lo as usize;
    let hi = lo + q_eff - 1;

    let mut lambda = (x - lo as f64).abs().max((hi as f64 - x).abs());
    if q > n {
        lambda += (q - n) as f64 / 2.0;
    }
    if !(lambda > 0.0) {
        lambda = 1.0;
    }

    let mut w = vec![0.0_f64; q_eff];
    let mut sw = 0.0;
    for i in lo..=hi {
        let u = (i as f64 - x).abs() / lambda;
        let tri = if u >= 0.999 {
            0.0
        } else {
            let a = 1.0 - u * u * u;
            a * a * a
        };
        let ww = tri * rw[i].max(0.0);
        w[i - lo] = ww;
        sw += ww;
    }
    if !(sw > 0.0) {
        return y[lo..=hi].iter().sum::<f64>() / q_eff as f64;
    }
    for v in w.iter_mut() {
        *v /= sw;
    }
    let mut yhat = 0.0;
    for i in lo..=hi {
        yhat += w[i - lo] * y[i];
    }
    if degree >= 1 {
        let mut xbar = 0.0;
        for i in lo..=hi {
            xbar += w[i - lo] * i as f64;
        }
        let mut num = 0.0;
        let mut den = 0.0;
        for i in lo..=hi {
            let dx = i as f64 - xbar;
            num += w[i - lo] * dx * y[i];
            den += w[i - lo] * dx * dx;
        }
        if den > 1e-12 {
            yhat += (num / den) * (x - xbar);
        }
    }
    yhat
}

fn loess_series(y: &[f64], rw: &[f64], q: usize, degree: usize) -> Vec<f64> {
    (0..y.len())
        .map(|i| loess_at(y, rw, q, degree, i as f64))
        .collect()
}

/// Simple moving average of order `w`; the result is `len - w + 1` long.
fn ma(x: &[f64], w: usize) -> Vec<f64> {
    if w == 0 || x.len() < w {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(x.len() - w + 1);
    let mut sum: f64 = x[..w].iter().sum();
    out.push(sum / w as f64);
    for i in w..x.len() {
        sum += x[i] - x[i - w];
        out.push(sum / w as f64);
    }
    out
}

fn median_abs(v: &[f64]) -> f64 {
    let mut a: Vec<f64> = v.iter().filter(|x| x.is_finite()).map(|x| x.abs()).collect();
    if a.is_empty() {
        return 0.0;
    }
    a.sort_by(|x, y| x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal));
    let m = a.len();
    if m % 2 == 1 {
        a[m / 2]
    } else {
        (a[m / 2 - 1] + a[m / 2]) / 2.0
    }
}

/// STL inner/outer loop. `seasonal_window == 0` means the `periodic` smoother.
fn stl(
    y: &[f64],
    period: usize,
    seasonal_window: usize,
    trend_window: usize,
    robust: bool,
) -> (Vec<f64>, Vec<f64>) {
    let n = y.len();
    let mut trend = vec![0.0; n];
    let mut seasonal = vec![0.0; n];
    let mut rw = vec![1.0; n];
    let low_pass = {
        let mut k = period + 1;
        if k % 2 == 0 {
            k += 1;
        }
        k.max(3)
    };
    let (inner, outer) = if robust { (1, 15) } else { (2, 0) };

    for pass in 0..=outer {
        for _ in 0..inner {
            // 1. detrend
            let det: Vec<f64> = (0..n).map(|i| y[i] - trend[i]).collect();

            // 2. cycle-subseries smoothing, extended one cycle either side
            let mut c = vec![0.0_f64; n + 2 * period];
            for k in 0..period {
                let idx: Vec<usize> = (k..n).step_by(period).collect();
                let sub: Vec<f64> = idx.iter().map(|i| det[*i]).collect();
                let subw: Vec<f64> = idx.iter().map(|i| rw[*i]).collect();
                let m = sub.len();
                if seasonal_window == 0 {
                    // `periodic`: one weighted mean per cycle position.
                    let sw: f64 = subw.iter().sum();
                    let mean = if sw > 0.0 {
                        sub.iter().zip(&subw).map(|(v, w)| v * w).sum::<f64>() / sw
                    } else {
                        sub.iter().sum::<f64>() / m as f64
                    };
                    for j in 0..m + 2 {
                        c[k + j * period] = mean;
                    }
                } else {
                    for j in 0..m + 2 {
                        c[k + j * period] =
                            loess_at(&sub, &subw, seasonal_window, 1, j as f64 - 1.0);
                    }
                }
            }

            // 3. low-pass filter of the extended seasonal series
            let l1 = ma(&c, period);
            let l2 = ma(&l1, period);
            let l3 = ma(&l2, 3);
            let ones = vec![1.0; l3.len()];
            let low = loess_series(&l3, &ones, low_pass, 1);

            // 4. seasonal = extended cycle-subseries minus the low-pass part
            for i in 0..n {
                seasonal[i] = c[i + period] - low.get(i).copied().unwrap_or(0.0);
            }

            // 5-6. deseasonalize, then smooth for the trend
            let deseason: Vec<f64> = (0..n).map(|i| y[i] - seasonal[i]).collect();
            trend = loess_series(&deseason, &rw, trend_window, 1);
        }

        if pass < outer {
            let resid: Vec<f64> = (0..n).map(|i| y[i] - trend[i] - seasonal[i]).collect();
            let h = 6.0 * median_abs(&resid);
            if !(h > 0.0) {
                break;
            }
            for i in 0..n {
                let u = (resid[i].abs() / h).min(1.0);
                let a = 1.0 - u * u;
                rw[i] = a * a;
            }
        }
    }
    (trend, seasonal)
}

// ---------------------------------------------------------------------------
// diagnostics
// ---------------------------------------------------------------------------

fn variance(v: &[f64]) -> f64 {
    let vals: Vec<f64> = v.iter().copied().filter(|x| x.is_finite()).collect();
    if vals.len() < 2 {
        return f64::NAN;
    }
    let mean = vals.iter().sum::<f64>() / vals.len() as f64;
    vals.iter().map(|x| (x - mean) * (x - mean)).sum::<f64>() / (vals.len() - 1) as f64
}

/// fpp-style strength of trend and seasonality: `1 - Var(R) / Var(T + R)` and
/// `1 - Var(R) / Var(S + R)`, clamped to 0-1.
fn strengths(trend: &[f64], seasonal: &[f64], residual: &[f64]) -> (f64, f64) {
    let n = trend.len();
    let tr: Vec<f64> = (0..n).map(|i| trend[i] + residual[i]).collect();
    let sr: Vec<f64> = (0..n).map(|i| seasonal[i] + residual[i]).collect();
    let vr = variance(residual);
    let f = |v: f64| -> f64 {
        if !vr.is_finite() || !v.is_finite() || !(v > 0.0) {
            f64::NAN
        } else {
            (1.0 - vr / v).clamp(0.0, 1.0)
        }
    };
    (f(variance(&tr)), f(variance(&sr)))
}

// ---------------------------------------------------------------------------
// formatting helpers
// ---------------------------------------------------------------------------

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

/// Round to `p` decimals, then drop trailing zeros.
fn fmt_p(v: f64, p: u32) -> String {
    if !v.is_finite() {
        return "n/a".into();
    }
    let s = format!("{v:.prec$}", prec = p.min(12) as usize);
    let s = if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s
    };
    if s == "-0" {
        "0".into()
    } else {
        s
    }
}

fn fmt_tick(v: f64, step: f64) -> String {
    let decimals = if step >= 1.0 {
        0
    } else {
        (-step.log10().floor()) as usize
    }
    .min(6);
    let s = format!("{v:.decimals$}");
    let s = if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s
    };
    if s == "-0" {
        "0".into()
    } else {
        s
    }
}

fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
    out
}

fn json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
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
    out.push('"');
    out
}

fn json_num(v: f64, p: u32) -> String {
    if v.is_finite() {
        fmt_p(v, p)
    } else {
        "null".into()
    }
}

fn nice_step(range: f64, target: usize) -> f64 {
    if !(range > 0.0) || target == 0 {
        return 1.0;
    }
    let raw = range / target as f64;
    let mag = 10f64.powf(raw.log10().floor());
    let norm = raw / mag;
    let mult = if norm <= 1.0 {
        1.0
    } else if norm <= 2.0 {
        2.0
    } else if norm <= 5.0 {
        5.0
    } else {
        10.0
    };
    mult * mag
}

fn fmt_px(v: f64) -> String {
    let r = (v * 100.0).round() / 100.0;
    let s = format!("{r}");
    if s == "-0" {
        "0".into()
    } else {
        s
    }
}

fn csv_cell(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// One-line summary of what was actually run — shown on the chart and in the table.
fn summary_line(d: &Decomposition) -> String {
    let mut s = format!(
        "{} · {} · period {}",
        if d.method == "stl" { "STL" } else { "Classical" },
        d.model,
        d.period
    );
    if d.period_source == "auto" {
        s.push_str(" (auto)");
    }
    let _ = write!(s, " · n {}", d.n);
    if d.robust {
        s.push_str(" · robust");
    }
    s
}

fn x_tick_label(d: &Decomposition, i: usize) -> String {
    if d.labels.get(i).map(|l| !l.is_empty()).unwrap_or(false) {
        d.labels[i].clone()
    } else {
        (i + 1).to_string()
    }
}

// ---------------------------------------------------------------------------
// text + CSV + JSON output
// ---------------------------------------------------------------------------

fn render_table(d: &Decomposition, opts: &Options) -> String {
    let p = opts.precision;
    let mut out = String::with_capacity(1024);
    let _ = writeln!(out, "{}", summary_line(d));
    if d.method == "stl" {
        let _ = writeln!(
            out,
            "seasonal window: {}   trend window: {}",
            d.seasonal_window, d.trend_window
        );
    } else {
        let _ = writeln!(
            out,
            "moving average: {} ({})",
            d.trend_window,
            if opts.two_sided { "centred" } else { "trailing" }
        );
    }
    let _ = writeln!(
        out,
        "strength of trend: {}   strength of seasonality: {}",
        fmt_p(d.trend_strength, 4),
        fmt_p(d.seasonal_strength, 4)
    );
    out.push('\n');

    let has_labels = d.labels.iter().any(|l| !l.is_empty());
    let mut headers: Vec<String> = vec!["#".into()];
    if has_labels {
        headers.push("Label".into());
    }
    headers.extend(
        ["Observed", "Trend", "Seasonal", "Residual", "Adjusted"]
            .iter()
            .map(|s| s.to_string()),
    );

    let mut rows: Vec<Vec<String>> = Vec::with_capacity(d.n);
    for i in 0..d.n {
        let mut row = vec![(i + 1).to_string()];
        if has_labels {
            row.push(d.labels[i].clone());
        }
        row.push(fmt_p(d.observed[i], p));
        row.push(fmt_p(d.trend[i], p));
        row.push(fmt_p(d.seasonal[i], p));
        row.push(fmt_p(d.residual[i], p));
        row.push(fmt_p(d.adjusted[i], p));
        rows.push(row);
    }

    let cols = headers.len();
    let mut widths: Vec<usize> = headers.iter().map(|h| h.chars().count()).collect();
    for row in &rows {
        for c in 0..cols {
            widths[c] = widths[c].max(row[c].chars().count());
        }
    }
    let pad = |s: &str, w: usize| -> String {
        let mut t = s.to_string();
        while t.chars().count() < w {
            t.push(' ');
        }
        t
    };
    let header_line: Vec<String> = (0..cols).map(|c| pad(&headers[c], widths[c])).collect();
    let _ = writeln!(out, "{}", header_line.join("  ").trim_end());
    let rule: Vec<String> = (0..cols).map(|c| "-".repeat(widths[c])).collect();
    let _ = writeln!(out, "{}", rule.join("  "));
    for row in &rows {
        let line: Vec<String> = (0..cols).map(|c| pad(&row[c], widths[c])).collect();
        let _ = writeln!(out, "{}", line.join("  ").trim_end());
    }

    out.push('\n');
    let _ = writeln!(
        out,
        "Seasonal {} (average effect at each position in the cycle)",
        if d.model == "multiplicative" {
            "factors"
        } else {
            "indices"
        }
    );
    for (k, v) in d.seasonal_indices.iter().enumerate() {
        let _ = writeln!(out, "position {}: {}", k + 1, fmt_p(*v, p));
    }
    out
}

fn render_csv(d: &Decomposition, opts: &Options) -> String {
    let p = opts.precision;
    let mut out = String::with_capacity(64 * d.n);
    out.push_str("index,label,observed,trend,seasonal,residual,seasonally_adjusted\n");
    for i in 0..d.n {
        let _ = writeln!(
            out,
            "{},{},{},{},{},{},{}",
            i + 1,
            csv_cell(d.labels.get(i).map(String::as_str).unwrap_or("")),
            fmt_p(d.observed[i], p),
            fmt_p(d.trend[i], p),
            fmt_p(d.seasonal[i], p),
            fmt_p(d.residual[i], p),
            fmt_p(d.adjusted[i], p)
        );
    }
    out
}

fn render_json(d: &Decomposition, opts: &Options) -> String {
    let p = opts.precision;
    let mut out = String::with_capacity(96 * d.n);
    out.push('{');
    let _ = write!(
        out,
        "\"method\":{},\"model\":{},\"period\":{},\"period_source\":{},\"n\":{}",
        json_str(&d.method),
        json_str(&d.model),
        d.period,
        json_str(&d.period_source),
        d.n
    );
    let _ = write!(
        out,
        ",\"seasonal_window\":{},\"trend_window\":{},\"robust\":{}",
        json_str(&d.seasonal_window),
        d.trend_window,
        d.robust
    );
    let _ = write!(
        out,
        ",\"trend_strength\":{},\"seasonal_strength\":{}",
        json_num(d.trend_strength, 4),
        json_num(d.seasonal_strength, 4)
    );
    out.push_str(",\"seasonal_indices\":[");
    for (k, v) in d.seasonal_indices.iter().enumerate() {
        if k > 0 {
            out.push(',');
        }
        out.push_str(&json_num(*v, p));
    }
    out.push_str("],\"points\":[");
    for i in 0..d.n {
        if i > 0 {
            out.push(',');
        }
        let _ = write!(
            out,
            "{{\"index\":{},\"label\":{},\"observed\":{},\"trend\":{},\"seasonal\":{},\"residual\":{},\"seasonally_adjusted\":{}}}",
            i + 1,
            json_str(d.labels.get(i).map(String::as_str).unwrap_or("")),
            json_num(d.observed[i], p),
            json_num(d.trend[i], p),
            json_num(d.seasonal[i], p),
            json_num(d.residual[i], p),
            json_num(d.adjusted[i], p)
        );
    }
    out.push_str("]}");
    out
}

// ---------------------------------------------------------------------------
// SVG
// ---------------------------------------------------------------------------

struct Theme {
    bg: &'static str,
    text: &'static str,
    muted: &'static str,
    axis: &'static str,
    grid: &'static str,
    accent: &'static str,
}

fn theme_of(name: &str) -> Theme {
    match name {
        "dark" => Theme {
            bg: "#0f172a",
            text: "#e2e8f0",
            muted: "#94a3b8",
            axis: "#475569",
            grid: "#1e293b",
            accent: "#f87171",
        },
        _ => Theme {
            bg: "#ffffff",
            text: "#0f172a",
            muted: "#475569",
            axis: "#94a3b8",
            grid: "#e2e8f0",
            accent: "#dc2626",
        },
    }
}

/// `M x y L x y …`, starting a fresh subpath wherever the series has a gap.
fn line_path(values: &[f64], xpx: &dyn Fn(usize) -> f64, ypx: &dyn Fn(f64) -> f64) -> String {
    let mut p = String::new();
    let mut pen_down = false;
    for (i, v) in values.iter().enumerate() {
        if !v.is_finite() {
            pen_down = false;
            continue;
        }
        let cmd = if pen_down { 'L' } else { 'M' };
        let _ = write!(p, "{}{} {}", cmd, fmt_px(xpx(i)), fmt_px(ypx(*v)));
        p.push(' ');
        pen_down = true;
    }
    p.trim_end().to_string()
}

fn render_svg(d: &Decomposition, opts: &Options) -> String {
    let theme = theme_of(normalize_opt(&opts.theme, "light"));
    let color = normalize_opt(&opts.color, "#2563eb");
    let p = opts.precision;
    let w = opts.width.clamp(360, 2400) as f64;
    let h = opts.height.clamp(320, 2400) as f64;
    let title = opts.title.trim();
    let x_label = opts.x_label.trim();
    let y_label = if opts.y_label.trim().is_empty() {
        "Value"
    } else {
        opts.y_label.trim()
    };
    let bars = normalize_opt(&opts.residual_style, "bar") != "line";

    let pad_top = if title.is_empty() { 40.0 } else { 66.0 };
    let pad_left = 78.0;
    let pad_right = 22.0;
    let pad_bottom = if x_label.is_empty() { 46.0 } else { 66.0 };
    let gap = 26.0;
    let plot_w = (w - pad_left - pad_right).max(80.0);
    let plot_h = (h - pad_top - pad_bottom).max(160.0);
    let panel_h = (plot_h - 3.0 * gap) / 4.0;
    let x0 = pad_left;
    let x1 = x0 + plot_w;

    let n = d.n;
    let xpx = |i: usize| -> f64 {
        if n > 1 {
            x0 + (i as f64) / ((n - 1) as f64) * plot_w
        } else {
            x0 + plot_w / 2.0
        }
    };

    let mut s = String::with_capacity(8192);
    let _ = write!(
        s,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}" role="img" font-family="{FONT}">"#,
        w = fmt_px(w),
        h = fmt_px(h),
    );
    let _ = write!(
        s,
        "<title>{}</title>",
        esc(if title.is_empty() {
            "Time series decomposition"
        } else {
            title
        })
    );
    let _ = write!(
        s,
        "<desc>{} decomposition, {} model, period {}, {} points, strength of trend {}, strength of seasonality {}</desc>",
        if d.method == "stl" { "STL" } else { "Classical" },
        esc(&d.model),
        d.period,
        d.n,
        fmt_p(d.trend_strength, 4),
        fmt_p(d.seasonal_strength, 4)
    );
    let _ = write!(
        s,
        r#"<rect width="{}" height="{}" fill="{}"/>"#,
        fmt_px(w),
        fmt_px(h),
        theme.bg
    );

    if !title.is_empty() {
        let _ = write!(
            s,
            r#"<text x="{}" y="30" text-anchor="middle" font-size="18" font-weight="600" fill="{}">{}</text>"#,
            fmt_px(w / 2.0),
            theme.text,
            esc(title)
        );
    }
    let _ = write!(
        s,
        r#"<text x="{}" y="{}" text-anchor="middle" font-size="12" fill="{}">{}</text>"#,
        fmt_px(w / 2.0),
        fmt_px(pad_top - 14.0),
        theme.muted,
        esc(&summary_line(d))
    );
    let _ = write!(
        s,
        r#"<text transform="rotate(-90 16 {cy})" x="16" y="{cy}" text-anchor="middle" font-size="12" fill="{}">{}</text>"#,
        theme.muted,
        esc(y_label),
        cy = fmt_px(pad_top + plot_h / 2.0),
    );

    let panels: [(&str, &Vec<f64>); 4] = [
        ("Observed", &d.observed),
        ("Trend", &d.trend),
        ("Seasonal", &d.seasonal),
        ("Residual", &d.residual),
    ];

    for (k, (name, series)) in panels.iter().enumerate() {
        let top = pad_top + (k as f64) * (panel_h + gap);
        let bottom = top + panel_h;

        // Panel value range, widened for anything drawn on top of it.
        let mut lo = f64::INFINITY;
        let mut hi = f64::NEG_INFINITY;
        let mut consider = |v: f64| {
            if v.is_finite() {
                lo = lo.min(v);
                hi = hi.max(v);
            }
        };
        for v in series.iter() {
            consider(*v);
        }
        if k == 0 {
            if opts.trend_overlay {
                for v in d.trend.iter() {
                    consider(*v);
                }
            }
            if opts.show_adjusted {
                for v in d.adjusted.iter() {
                    consider(*v);
                }
            }
        }
        if k == 3 {
            let zero = if d.model == "multiplicative" { 1.0 } else { 0.0 };
            consider(zero);
        }
        if !lo.is_finite() || !hi.is_finite() {
            lo = 0.0;
            hi = 1.0;
        }
        if (hi - lo).abs() < f64::EPSILON {
            lo -= 0.5;
            hi += 0.5;
        }
        let step = nice_step(hi - lo, 3);
        let lo_t = (lo / step).floor() * step;
        let hi_t = (hi / step).ceil() * step;
        let span = (hi_t - lo_t).max(f64::MIN_POSITIVE);
        let ypx = move |v: f64| -> f64 { bottom - ((v - lo_t) / span) * panel_h };

        // gridlines + value ticks
        let mut t = lo_t;
        while t <= hi_t + step * 0.5 {
            let y = ypx(t);
            if opts.grid {
                let _ = write!(
                    s,
                    r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="1"/>"#,
                    fmt_px(x0),
                    fmt_px(y),
                    fmt_px(x1),
                    fmt_px(y),
                    theme.grid
                );
            }
            let _ = write!(
                s,
                r#"<text x="{}" y="{}" text-anchor="end" font-size="10" fill="{}">{}</text>"#,
                fmt_px(x0 - 8.0),
                fmt_px(y + 3.5),
                theme.muted,
                esc(&fmt_tick(t, step))
            );
            t += step;
        }

        // axis frame
        let _ = write!(
            s,
            r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="1"/>"#,
            fmt_px(x0),
            fmt_px(bottom),
            fmt_px(x1),
            fmt_px(bottom),
            theme.axis
        );
        let _ = write!(
            s,
            r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="1"/>"#,
            fmt_px(x0),
            fmt_px(top),
            fmt_px(x0),
            fmt_px(bottom),
            theme.axis
        );

        // panel body
        if k == 3 && bars {
            let base = if d.model == "multiplicative" { 1.0 } else { 0.0 };
            let bw = if n > 1 {
                (plot_w / n as f64 * 0.6).clamp(1.0, 14.0)
            } else {
                14.0
            };
            let y_base = ypx(base);
            for (i, v) in series.iter().enumerate() {
                if !v.is_finite() {
                    continue;
                }
                let y = ypx(*v);
                let (ry, rh) = if y < y_base {
                    (y, y_base - y)
                } else {
                    (y_base, y - y_base)
                };
                let _ = write!(
                    s,
                    r#"<rect x="{}" y="{}" width="{}" height="{}" fill="{}" opacity="0.85"/>"#,
                    fmt_px(xpx(i) - bw / 2.0),
                    fmt_px(ry),
                    fmt_px(bw),
                    fmt_px(rh.max(0.75)),
                    color
                );
            }
            let _ = write!(
                s,
                r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="1" stroke-dasharray="3 3"/>"#,
                fmt_px(x0),
                fmt_px(y_base),
                fmt_px(x1),
                fmt_px(y_base),
                theme.muted
            );
        } else {
            let path = line_path(series, &xpx, &ypx);
            if !path.is_empty() {
                let _ = write!(
                    s,
                    r#"<path d="{path}" fill="none" stroke="{color}" stroke-width="1.8" stroke-linejoin="round"/>"#
                );
            }
        }

        if k == 0 {
            if opts.show_adjusted {
                let path = line_path(&d.adjusted, &xpx, &ypx);
                if !path.is_empty() {
                    let _ = write!(
                        s,
                        r#"<path d="{path}" fill="none" stroke="{}" stroke-width="1.4" stroke-dasharray="2 3" opacity="0.9"/>"#,
                        theme.muted
                    );
                }
            }
            if opts.trend_overlay {
                let path = line_path(&d.trend, &xpx, &ypx);
                if !path.is_empty() {
                    let _ = write!(
                        s,
                        r#"<path d="{path}" fill="none" stroke="{}" stroke-width="1.6" stroke-dasharray="6 4"/>"#,
                        theme.accent
                    );
                }
            }
        }

        // panel name
        let _ = write!(
            s,
            r#"<text x="{}" y="{}" font-size="12" font-weight="600" fill="{}">{}</text>"#,
            fmt_px(x0 + 6.0),
            fmt_px(top + 14.0),
            theme.text,
            esc(name)
        );
        if k == 0 && opts.trend_overlay {
            let _ = write!(
                s,
                r#"<text x="{}" y="{}" text-anchor="end" font-size="11" fill="{}">trend overlay</text>"#,
                fmt_px(x1 - 6.0),
                fmt_px(top + 14.0),
                theme.accent
            );
        }
        if k == 0 && opts.show_adjusted {
            let _ = write!(
                s,
                r#"<text x="{}" y="{}" text-anchor="end" font-size="11" fill="{}">seasonally adjusted</text>"#,
                fmt_px(x1 - 6.0),
                fmt_px(top + 28.0),
                theme.muted
            );
        }
    }

    // x ticks under the last panel
    let base_y = pad_top + plot_h;
    let want = 8usize;
    let stride = ((n as f64) / want as f64).ceil().max(1.0) as usize;
    let mut i = 0usize;
    while i < n {
        let x = xpx(i);
        let _ = write!(
            s,
            r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="1"/>"#,
            fmt_px(x),
            fmt_px(base_y),
            fmt_px(x),
            fmt_px(base_y + 4.0),
            theme.axis
        );
        let _ = write!(
            s,
            r#"<text x="{}" y="{}" text-anchor="middle" font-size="10" fill="{}">{}</text>"#,
            fmt_px(x),
            fmt_px(base_y + 17.0),
            theme.muted,
            esc(&x_tick_label(d, i))
        );
        i += stride;
    }

    if !x_label.is_empty() {
        let _ = write!(
            s,
            r#"<text x="{}" y="{}" text-anchor="middle" font-size="12" fill="{}">{}</text>"#,
            fmt_px(x0 + plot_w / 2.0),
            fmt_px(base_y + 38.0),
            theme.muted,
            esc(x_label)
        );
    }

    // Machine-readable footer so the chart carries its own numbers.
    let _ = write!(
        s,
        "<desc>seasonal {}: {}</desc>",
        if d.model == "multiplicative" {
            "factors"
        } else {
            "indices"
        },
        esc(
            &d.seasonal_indices
                .iter()
                .map(|v| fmt_p(*v, p))
                .collect::<Vec<_>>()
                .join(", ")
        )
    );
    s.push_str("</svg>");
    s
}

// ---------------------------------------------------------------------------
// tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// 4 years of monthly data: linear trend + a fixed 12-month seasonal pattern.
    fn monthly_series() -> Vec<f64> {
        let season = [
            4.0, 2.0, -1.0, -3.0, -5.0, -2.0, 1.0, 3.0, 6.0, 2.0, -3.0, -4.0,
        ];
        (0..48)
            .map(|i| 100.0 + 0.5 * i as f64 + season[i % 12])
            .collect()
    }

    fn as_text(v: &[f64]) -> String {
        v.iter()
            .map(|x| format!("{x}"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn stl_recovers_a_known_trend_and_seasonal_pattern() {
        let y = monthly_series();
        let opts = Options {
            period: 12,
            ..Options::default()
        };
        let d = analyze(&as_text(&y), &opts).unwrap();
        assert_eq!(d.n, 48);
        assert_eq!(d.period, 12);
        assert_eq!(d.method, "stl");
        assert_eq!(d.seasonal_window, "periodic");

        // The seasonal shape comes back (centred, so compare against the centred truth).
        let season = [
            4.0, 2.0, -1.0, -3.0, -5.0, -2.0, 1.0, 3.0, 6.0, 2.0, -3.0, -4.0,
        ];
        let mean = season.iter().sum::<f64>() / 12.0;
        for k in 0..12 {
            let want = season[k] - mean;
            assert!(
                (d.seasonal_indices[k] - want).abs() < 0.6,
                "position {k}: got {} want {want}",
                d.seasonal_indices[k]
            );
        }
        // The trend tracks the 0.5/step line.
        for i in 10..38 {
            let want = 100.0 + 0.5 * i as f64;
            assert!(
                (d.trend[i] - want).abs() < 1.0,
                "trend[{i}] = {} want ~{want}",
                d.trend[i]
            );
        }
        // Residuals are small and the additive identity holds exactly.
        for i in 0..48 {
            assert!(d.residual[i].abs() < 1.0, "residual[{i}] = {}", d.residual[i]);
            let sum = d.trend[i] + d.seasonal[i] + d.residual[i];
            assert!((sum - d.observed[i]).abs() < 1e-9, "identity broke at {i}");
        }
        assert!(d.seasonal_strength > 0.9, "{}", d.seasonal_strength);
        assert!(d.trend_strength > 0.9, "{}", d.trend_strength);
    }

    #[test]
    fn classical_matches_the_moving_average_definition() {
        let y = monthly_series();
        let opts = Options {
            method: "classical".into(),
            period: 12,
            ..Options::default()
        };
        let d = analyze(&as_text(&y), &opts).unwrap();
        // Centred 2x12 MA at i=6 over a clean linear trend equals the trend value.
        assert!((d.trend[6] - (100.0 + 0.5 * 6.0)).abs() < 1e-6, "{}", d.trend[6]);
        // Seasonal indices sum to zero after centring.
        let sum: f64 = d.seasonal_indices.iter().sum();
        assert!(sum.abs() < 1e-9, "indices should be centred, got {sum}");
        for i in 0..48 {
            let recon = d.trend[i] + d.seasonal[i] + d.residual[i];
            assert!((recon - d.observed[i]).abs() < 1e-9);
        }
    }

    #[test]
    fn classical_without_extrapolation_leaves_the_ends_blank() {
        let y = monthly_series();
        let opts = Options {
            method: "classical".into(),
            period: 12,
            extrapolate_trend: false,
            ..Options::default()
        };
        let d = analyze(&as_text(&y), &opts).unwrap();
        assert!(!d.trend[0].is_finite());
        assert!(!d.trend[47].is_finite());
        assert!(d.trend[24].is_finite());

        let filled = analyze(
            &as_text(&y),
            &Options {
                method: "classical".into(),
                period: 12,
                ..Options::default()
            },
        )
        .unwrap();
        assert!(filled.trend[0].is_finite());
        assert!(filled.trend[47].is_finite());
    }

    #[test]
    fn trailing_moving_average_starts_later_than_a_centred_one() {
        let y = monthly_series();
        let d = analyze(
            &as_text(&y),
            &Options {
                method: "classical".into(),
                period: 12,
                two_sided: false,
                extrapolate_trend: false,
                ..Options::default()
            },
        )
        .unwrap();
        assert!(!d.trend[10].is_finite());
        assert!(d.trend[11].is_finite());
        assert!(d.trend[47].is_finite(), "a trailing MA reaches the last point");
    }

    #[test]
    fn multiplicative_components_multiply_back_to_the_observations() {
        let season = [1.2, 0.9, 0.8, 1.1];
        let y: Vec<f64> = (0..40)
            .map(|i| (50.0 + 2.0 * i as f64) * season[i % 4])
            .collect();
        let d = analyze(
            &as_text(&y),
            &Options {
                model: "multiplicative".into(),
                period: 4,
                ..Options::default()
            },
        )
        .unwrap();
        for i in 0..40 {
            let recon = d.trend[i] * d.seasonal[i] * d.residual[i];
            assert!(
                (recon - d.observed[i]).abs() < 1e-6,
                "multiplicative identity broke at {i}: {recon} vs {}",
                d.observed[i]
            );
        }
        // Seasonal factors sit around 1 and keep the input's shape.
        assert!(d.seasonal_indices[0] > d.seasonal_indices[2]);
        assert!(d.adjusted[8] > 0.0);
    }

    #[test]
    fn multiplicative_rejects_non_positive_values() {
        let mut y = monthly_series();
        y[5] = 0.0;
        let err = analyze(
            &as_text(&y),
            &Options {
                model: "multiplicative".into(),
                period: 12,
                ..Options::default()
            },
        )
        .unwrap_err();
        assert!(err.contains("greater than 0"), "{err}");
        assert!(err.contains("position 6"), "{err}");
    }

    #[test]
    fn period_is_detected_automatically() {
        let y = monthly_series();
        assert_eq!(detect_period(&y), Some(12));
        let d = analyze(&as_text(&y), &Options::default()).unwrap();
        assert_eq!(d.period, 12);
        assert_eq!(d.period_source, "auto");
    }

    #[test]
    fn a_weekly_cycle_is_detected_as_period_7() {
        let season = [3.0, 1.0, 0.0, -1.0, -2.0, 5.0, 6.0];
        let y: Vec<f64> = (0..70)
            .map(|i| 20.0 + 0.1 * i as f64 + season[i % 7])
            .collect();
        assert_eq!(detect_period(&y), Some(7));
    }

    #[test]
    fn a_trend_only_series_reports_that_no_period_was_found() {
        let y: Vec<f64> = (0..40).map(|i| 10.0 + 0.7 * i as f64).collect();
        let err = analyze(&as_text(&y), &Options::default()).unwrap_err();
        assert!(err.contains("could not detect a seasonal period"), "{err}");
        assert!(err.contains("set period explicitly"), "{err}");
    }

    #[test]
    fn robust_stl_holds_the_seasonal_shape_through_an_outlier() {
        let mut y = monthly_series();
        y[20] += 60.0; // one bad reading
        let plain = analyze(
            &as_text(&y),
            &Options {
                period: 12,
                ..Options::default()
            },
        )
        .unwrap();
        let robust = analyze(
            &as_text(&y),
            &Options {
                period: 12,
                robust: true,
                ..Options::default()
            },
        )
        .unwrap();
        assert!(robust.robust);
        // The outlier should be pushed into the residual rather than the trend.
        assert!(
            robust.residual[20].abs() > plain.residual[20].abs(),
            "robust residual {} vs plain {}",
            robust.residual[20],
            plain.residual[20]
        );
        let clean = analyze(
            &as_text(&monthly_series()),
            &Options {
                period: 12,
                ..Options::default()
            },
        )
        .unwrap();
        let err_robust: f64 = (0..12)
            .map(|k| (robust.seasonal_indices[k] - clean.seasonal_indices[k]).abs())
            .sum();
        let err_plain: f64 = (0..12)
            .map(|k| (plain.seasonal_indices[k] - clean.seasonal_indices[k]).abs())
            .sum();
        assert!(
            err_robust <= err_plain,
            "robust {err_robust} should not be worse than plain {err_plain}"
        );
    }

    #[test]
    fn a_numeric_seasonal_window_lets_the_pattern_drift() {
        let y = monthly_series();
        let d = analyze(
            &as_text(&y),
            &Options {
                period: 12,
                seasonal_window: 7,
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(d.seasonal_window, "7");
        assert!(d.trend_window >= 19, "trend window {}", d.trend_window);
        // Even seasonal windows are nudged up to the next odd length.
        let e = analyze(
            &as_text(&y),
            &Options {
                period: 12,
                seasonal_window: 8,
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(e.seasonal_window, "9");
    }

    #[test]
    fn label_value_rows_are_parsed_and_carried_through() {
        let mut text = String::from("month,sales\n");
        for (i, v) in monthly_series().iter().enumerate() {
            let _ = writeln!(text, "2022-{:02},{v}", i % 12 + 1);
        }
        let d = analyze(
            &text,
            &Options {
                period: 12,
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(d.n, 48);
        assert_eq!(d.labels[0], "2022-01");
        let csv = render(
            &text,
            &Options {
                period: 12,
                output: "csv".into(),
                ..Options::default()
            },
        )
        .unwrap();
        assert!(csv.starts_with("index,label,observed,trend,seasonal,residual,seasonally_adjusted\n"));
        assert!(csv.contains(",2022-01,"), "{}", &csv[..120]);
    }

    #[test]
    fn a_single_line_of_comma_separated_values_works() {
        let y = monthly_series();
        let text = y
            .iter()
            .map(|v| format!("{v}"))
            .collect::<Vec<_>>()
            .join(", ");
        let d = analyze(
            &text,
            &Options {
                period: 12,
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(d.n, 48);
    }

    #[test]
    fn non_numeric_data_is_rejected_with_the_line_number() {
        let text = "10\n12\nbanana\n14\n16\n18";
        let err = analyze(
            text,
            &Options {
                period: 2,
                ..Options::default()
            },
        )
        .unwrap_err();
        assert_eq!(err, "line 3: 'banana' is not a number");
    }

    #[test]
    fn too_few_values_and_too_few_cycles_are_rejected() {
        let err = analyze("1\n2\n3", &Options::default()).unwrap_err();
        assert!(err.contains("at least 4 numeric values"), "{err}");

        let y = monthly_series();
        let short: Vec<f64> = y[..18].to_vec();
        let err = analyze(
            &as_text(&short),
            &Options {
                period: 12,
                ..Options::default()
            },
        )
        .unwrap_err();
        assert!(err.contains("2 full cycles"), "{err}");
        assert!(err.contains("requires 24 values, got 18"), "{err}");
    }

    #[test]
    fn bad_enum_values_are_named_in_the_error() {
        let y = as_text(&monthly_series());
        let err = analyze(
            &y,
            &Options {
                method: "wavelet".into(),
                period: 12,
                ..Options::default()
            },
        )
        .unwrap_err();
        assert!(err.contains("unknown method 'wavelet'"), "{err}");

        let err = analyze(
            &y,
            &Options {
                model: "hybrid".into(),
                period: 12,
                ..Options::default()
            },
        )
        .unwrap_err();
        assert!(err.contains("unknown model 'hybrid'"), "{err}");

        let err = render(
            &y,
            &Options {
                period: 12,
                output: "png".into(),
                ..Options::default()
            },
        )
        .unwrap_err();
        assert!(err.contains("unknown output 'png'"), "{err}");

        let err = analyze(
            &y,
            &Options {
                period: 1,
                ..Options::default()
            },
        )
        .unwrap_err();
        assert!(err.contains("period must be at least 2"), "{err}");
    }

    #[test]
    fn svg_output_has_four_labelled_panels_and_real_metadata() {
        let y = as_text(&monthly_series());
        let svg = render(
            &y,
            &Options {
                period: 12,
                title: "Monthly sales".into(),
                x_label: "Month".into(),
                width: 900,
                height: 720,
                ..Options::default()
            },
        )
        .unwrap();
        assert!(svg.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"900\" height=\"720\""));
        assert!(svg.ends_with("</svg>"));
        for panel in ["Observed", "Trend", "Seasonal", "Residual"] {
            assert!(svg.contains(&format!(">{panel}</text>")), "missing {panel}");
        }
        assert!(svg.contains("Monthly sales"));
        assert!(svg.contains("STL · additive · period 12 · n 48"));
        assert!(svg.contains("STL decomposition, additive model, period 12, 48 points"));
        assert!(svg.contains("trend overlay"));
        assert!(svg.contains("<rect"), "residual bars are drawn by default");

        let line_style = render(
            &y,
            &Options {
                period: 12,
                residual_style: "line".into(),
                trend_overlay: false,
                grid: false,
                ..Options::default()
            },
        )
        .unwrap();
        assert!(!line_style.contains("trend overlay"));
        // Only the background rect survives when the residual panel is a line.
        assert_eq!(line_style.matches("<rect").count(), 1);
    }

    #[test]
    fn table_and_json_carry_the_diagnostics() {
        let y = as_text(&monthly_series());
        let table = render(
            &y,
            &Options {
                period: 12,
                output: "table".into(),
                precision: 2,
                ..Options::default()
            },
        )
        .unwrap();
        assert!(table.contains("STL · additive · period 12 · n 48"));
        assert!(table.contains("strength of trend:"));
        assert!(table.contains("Observed"));
        assert!(table.contains("position 1:"));

        let json = render(
            &y,
            &Options {
                period: 12,
                output: "json".into(),
                ..Options::default()
            },
        )
        .unwrap();
        assert!(json.contains("\"method\":\"stl\""));
        assert!(json.contains("\"period\":12"));
        assert!(json.contains("\"period_source\":\"explicit\""));
        assert!(json.contains("\"seasonal_window\":\"periodic\""));
        assert!(json.contains("\"points\":["));
        assert_eq!(json.matches("\"index\":").count(), 48);
    }

    #[test]
    fn seasonally_adjusted_removes_the_seasonal_component() {
        let y = monthly_series();
        let d = analyze(
            &as_text(&y),
            &Options {
                period: 12,
                ..Options::default()
            },
        )
        .unwrap();
        for i in 0..48 {
            assert!((d.adjusted[i] - (d.observed[i] - d.seasonal[i])).abs() < 1e-9);
        }
    }
}
