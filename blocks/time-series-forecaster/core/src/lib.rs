//! time-series-forecaster core — pure compute, shared by the chat skill block and the web page.
//! No wafer/wasm-bindgen deps.
//!
//! Forecasts a univariate time series with the exponential-smoothing (ETS) family:
//! simple exponential smoothing, Holt's linear trend, damped trend, and Holt-Winters
//! additive/multiplicative seasonality. Smoothing parameters left unset are fitted by a
//! deterministic coarse-to-fine grid search that minimises the sum of squared one-step-ahead
//! errors; `model = "auto"` then picks between the candidate models by AICc.
//!
//! Prediction intervals use the additive-error ETS forecast-variance recursion
//! `sigma_h^2 = sigma^2 * (1 + sum_{j=1}^{h-1} c_j^2)` with
//! `c_j = alpha + beta*j` (linear trend), `c_j = alpha + beta*phi*(1-phi^j)/(1-phi)` (damped)
//! and `+ gamma` on every `j` that is a multiple of the season length. Multiplicative
//! seasonality has no closed form, so its bands scale a relative residual sigma by the point
//! forecast — an approximation, flagged in the output notes.
//!
//! Everything is deterministic f64 arithmetic with no RNG and no clock, so the chat block,
//! the CLI and the browser page all return byte-identical results.

use serde_json::json;

/// Largest accepted number of observations.
pub const MAX_POINTS: usize = 10_000;
/// Smallest accepted number of observations.
pub const MIN_POINTS: usize = 3;
/// Largest accepted forecast horizon, in periods.
pub const MAX_HORIZON: i64 = 240;
/// Largest accepted season length, in periods.
pub const MAX_SEASON: i64 = 366;

/// Every knob the tool exposes. `Default` mirrors the descriptor defaults.
#[derive(Clone, Debug)]
pub struct Options {
    pub model: String,
    pub horizon: i64,
    pub season_length: i64,
    pub alpha: f64,
    pub beta: f64,
    pub gamma: f64,
    pub phi: f64,
    pub confidence: String,
    pub show_fitted: bool,
    pub header: String,
    pub decimals: i64,
    pub format: String,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            model: "auto".into(),
            horizon: 6,
            season_length: 0,
            alpha: 0.0,
            beta: 0.0,
            gamma: 0.0,
            phi: 0.0,
            confidence: "95".into(),
            show_fitted: false,
            header: "auto".into(),
            decimals: 3,
            format: "text".into(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Trend {
    None,
    Additive,
    Damped,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Seasonal {
    None,
    Additive,
    Multiplicative,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Spec {
    key: &'static str,
    label: &'static str,
    trend: Trend,
    seasonal: Seasonal,
}

const SPECS: [Spec; 5] = [
    Spec {
        key: "simple",
        label: "Simple exponential smoothing",
        trend: Trend::None,
        seasonal: Seasonal::None,
    },
    Spec {
        key: "holt",
        label: "Holt linear trend",
        trend: Trend::Additive,
        seasonal: Seasonal::None,
    },
    Spec {
        key: "damped",
        label: "Damped trend",
        trend: Trend::Damped,
        seasonal: Seasonal::None,
    },
    Spec {
        key: "holt-winters-additive",
        label: "Holt-Winters additive seasonality",
        trend: Trend::Additive,
        seasonal: Seasonal::Additive,
    },
    Spec {
        key: "holt-winters-multiplicative",
        label: "Holt-Winters multiplicative seasonality",
        trend: Trend::Additive,
        seasonal: Seasonal::Multiplicative,
    },
];

/// A fitted model: the smoothing weights, the final states and the in-sample fit.
struct Fit {
    spec: Spec,
    alpha: f64,
    beta: f64,
    gamma: f64,
    phi: f64,
    level: f64,
    trend: f64,
    seas: Vec<f64>,
    fitted: Vec<Option<f64>>,
    sse: f64,
    n_fit: usize,
}

impl Fit {
    fn sigma(&self) -> f64 {
        (self.sse / self.n_fit as f64).sqrt()
    }

    /// Number of free parameters + initial states, for AIC/AICc.
    fn k(&self, m: usize) -> f64 {
        let mut k = 1.0 + 1.0; // alpha + initial level
        if self.spec.trend != Trend::None {
            k += 2.0; // beta + initial trend
        }
        if self.spec.trend == Trend::Damped {
            k += 1.0; // phi
        }
        if self.spec.seasonal != Seasonal::None {
            k += 1.0 + m as f64; // gamma + initial seasonal indices
        }
        k
    }

    fn aicc(&self, m: usize) -> f64 {
        let n = self.n_fit as f64;
        let k = self.k(m);
        let aic = n * (self.sse / n).ln() + 2.0 * k;
        if n - k - 1.0 > 0.0 {
            aic + 2.0 * k * (k + 1.0) / (n - k - 1.0)
        } else {
            f64::INFINITY
        }
    }
}

// ---------------------------------------------------------------- parsing

/// Parse one token into a finite number, tolerating currency symbols, percent signs,
/// underscores, stray spaces and accounting-style `(1.5)` negatives.
fn clean_number(tok: &str) -> Option<f64> {
    let stripped: String = tok
        .trim()
        .chars()
        .filter(|c| !matches!(c, '$' | '€' | '£' | '¥' | '%' | '_' | ' ' | '\'' | '"'))
        .collect();
    if stripped.is_empty() {
        return None;
    }
    let normalized = if stripped.starts_with('(') && stripped.ends_with(')') {
        format!("-{}", &stripped[1..stripped.len() - 1])
    } else {
        stripped
    };
    normalized.parse::<f64>().ok().filter(|v| v.is_finite())
}

/// Split one row into fields. Commas win, then semicolons, then tabs, then whitespace —
/// so `Jan 2024, 120` keeps its label intact.
fn split_fields(line: &str) -> Vec<String> {
    let sep: char = if line.contains(',') {
        ','
    } else if line.contains(';') {
        ';'
    } else if line.contains('\t') {
        '\t'
    } else {
        ' '
    };
    line.split(sep)
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

struct Series {
    labels: Vec<String>,
    values: Vec<f64>,
    has_labels: bool,
}

fn parse_series(data: &str, header: &str) -> Result<Series, String> {
    let header = header.trim().to_ascii_lowercase();
    if !matches!(header.as_str(), "auto" | "yes" | "no") {
        return Err(format!(
            "header must be one of auto, yes, no (got \"{header}\")"
        ));
    }
    let rows: Vec<&str> = data
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();
    if rows.is_empty() {
        return Err("no data: paste at least 3 numeric observations, one per line or comma separated".into());
    }

    let mut labels: Vec<String> = Vec::new();
    let mut values: Vec<f64> = Vec::new();
    let mut has_labels = false;

    if rows.len() == 1 {
        // One row: the whole series lives on a single comma/space separated line.
        let mut fields = split_fields(rows[0]);
        let drop_first = match header.as_str() {
            "yes" => true,
            "no" => false,
            _ => fields.first().map(|f| clean_number(f).is_none()).unwrap_or(false),
        };
        if drop_first && !fields.is_empty() {
            fields.remove(0);
        }
        for (i, f) in fields.iter().enumerate() {
            match clean_number(f) {
                Some(v) => values.push(v),
                None => {
                    return Err(format!(
                        "value {} (\"{}\") is not a number — remove text and units, or set header=yes if the first entry is a label",
                        i + 1,
                        f
                    ))
                }
            }
        }
    } else {
        let value_of = |line: &str| -> Option<f64> {
            let f = split_fields(line);
            f.last().and_then(|t| clean_number(t))
        };
        let drop_first = match header.as_str() {
            "yes" => true,
            "no" => false,
            _ => value_of(rows[0]).is_none(),
        };
        let body = if drop_first { &rows[1..] } else { &rows[..] };
        for (i, line) in body.iter().enumerate() {
            let fields = split_fields(line);
            if fields.is_empty() {
                continue;
            }
            let raw = fields.last().unwrap();
            let v = clean_number(raw).ok_or_else(|| {
                format!(
                    "row {} value \"{}\" is not a number — keep one numeric value per row (an optional label may come first), or set header=yes to skip a title row",
                    i + 1,
                    raw
                )
            })?;
            if fields.len() > 1 {
                has_labels = true;
                labels.push(fields[..fields.len() - 1].join(" "));
            } else {
                labels.push(String::new());
            }
            values.push(v);
        }
    }

    if values.len() < MIN_POINTS {
        return Err(format!(
            "need at least {MIN_POINTS} observations to forecast (got {})",
            values.len()
        ));
    }
    if values.len() > MAX_POINTS {
        return Err(format!(
            "too many observations: {} (maximum {MAX_POINTS})",
            values.len()
        ));
    }
    if labels.len() < values.len() {
        labels.resize(values.len(), String::new());
    }
    Ok(Series {
        labels,
        values,
        has_labels,
    })
}

// ---------------------------------------------------------------- fitting

/// Run the ETS recursion for one fixed parameter set. Returns `None` when the recursion is
/// undefined or diverges (non-finite states, zero seasonal factors, too little data).
fn simulate(
    y: &[f64],
    m: usize,
    spec: Spec,
    alpha: f64,
    beta: f64,
    gamma: f64,
    phi: f64,
) -> Option<Fit> {
    let n = y.len();
    let seasonal_on = spec.seasonal != Seasonal::None;
    let mut seas = vec![0.0f64; n + m.max(1)];
    let mut level: f64;
    let mut trend: f64;
    let start: usize;

    if seasonal_on {
        if m < 2 || n < 2 * m {
            return None;
        }
        let cycles = n / m;
        let cycle_mean: Vec<f64> = (0..cycles)
            .map(|k| y[k * m..(k + 1) * m].iter().sum::<f64>() / m as f64)
            .collect();
        level = cycle_mean[0];
        trend = (cycle_mean[1] - cycle_mean[0]) / m as f64;
        for i in 0..m {
            let mut acc = 0.0;
            for (k, cm) in cycle_mean.iter().enumerate() {
                match spec.seasonal {
                    Seasonal::Additive => acc += y[k * m + i] - cm,
                    Seasonal::Multiplicative => {
                        if cm.abs() < 1e-12 {
                            return None;
                        }
                        acc += y[k * m + i] / cm;
                    }
                    Seasonal::None => {}
                }
            }
            seas[i] = acc / cycles as f64;
        }
        let mean = seas[..m].iter().sum::<f64>() / m as f64;
        match spec.seasonal {
            Seasonal::Additive => {
                for s in seas[..m].iter_mut() {
                    *s -= mean;
                }
            }
            Seasonal::Multiplicative => {
                if mean.abs() < 1e-12 {
                    return None;
                }
                for s in seas[..m].iter_mut() {
                    *s /= mean;
                    if s.abs() < 1e-9 {
                        return None;
                    }
                }
            }
            Seasonal::None => {}
        }
        start = 0;
    } else {
        level = y[0];
        trend = if spec.trend != Trend::None { y[1] - y[0] } else { 0.0 };
        start = 1;
    }

    let phi_eff = match spec.trend {
        Trend::None => 0.0,
        Trend::Additive => 1.0,
        Trend::Damped => phi,
    };

    let mut fitted: Vec<Option<f64>> = vec![None; n];
    let mut sse = 0.0f64;
    let mut n_fit = 0usize;

    for t in start..n {
        let f = level + phi_eff * trend;
        let s_prev = if seasonal_on { seas[t] } else { 0.0 };
        let yhat = match spec.seasonal {
            Seasonal::None => f,
            Seasonal::Additive => f + s_prev,
            Seasonal::Multiplicative => f * s_prev,
        };
        if !yhat.is_finite() {
            return None;
        }
        let e = y[t] - yhat;
        sse += e * e;
        n_fit += 1;
        fitted[t] = Some(yhat);

        let new_level = match spec.seasonal {
            Seasonal::None => alpha * y[t] + (1.0 - alpha) * f,
            Seasonal::Additive => alpha * (y[t] - s_prev) + (1.0 - alpha) * f,
            Seasonal::Multiplicative => {
                if s_prev.abs() < 1e-9 {
                    return None;
                }
                alpha * (y[t] / s_prev) + (1.0 - alpha) * f
            }
        };
        let new_trend = if spec.trend == Trend::None {
            0.0
        } else {
            beta * (new_level - level) + (1.0 - beta) * phi_eff * trend
        };
        if seasonal_on {
            let ns = match spec.seasonal {
                Seasonal::Additive => gamma * (y[t] - f) + (1.0 - gamma) * s_prev,
                Seasonal::Multiplicative => {
                    if f.abs() < 1e-9 {
                        return None;
                    }
                    gamma * (y[t] / f) + (1.0 - gamma) * s_prev
                }
                Seasonal::None => 0.0,
            };
            if !ns.is_finite() || (spec.seasonal == Seasonal::Multiplicative && ns.abs() < 1e-9) {
                return None;
            }
            seas[t + m] = ns;
        }
        if !new_level.is_finite() || !new_trend.is_finite() {
            return None;
        }
        level = new_level;
        trend = new_trend;
    }

    if n_fit == 0 || !sse.is_finite() {
        return None;
    }
    Some(Fit {
        spec,
        alpha,
        beta,
        gamma,
        phi: phi_eff.max(0.0),
        level,
        trend,
        seas,
        fitted,
        sse,
        n_fit,
    })
}

fn grid(fixed: Option<f64>, lo: f64, hi: f64, k: usize) -> Vec<f64> {
    if let Some(v) = fixed {
        return vec![v];
    }
    if k < 2 || hi <= lo {
        return vec![(lo + hi) / 2.0];
    }
    (0..k)
        .map(|i| lo + (hi - lo) * i as f64 / (k - 1) as f64)
        .collect()
}

fn shrink(best: f64, lo: f64, hi: f64, k: usize, floor: f64, ceil: f64) -> (f64, f64) {
    let step = (hi - lo) / (k - 1).max(1) as f64;
    ((best - step).max(floor), (best + step).min(ceil))
}

/// Coarse-to-fine grid search minimising the sum of squared one-step-ahead errors.
/// Any parameter the caller pinned (> 0) is held fixed.
fn optimize(y: &[f64], m: usize, spec: Spec, opts: &Options) -> Option<Fit> {
    let alpha_fixed = (opts.alpha > 0.0).then_some(opts.alpha);
    let beta_fixed = (opts.beta > 0.0).then_some(opts.beta);
    let gamma_fixed = (opts.gamma > 0.0).then_some(opts.gamma);
    let phi_fixed = (opts.phi > 0.0).then_some(opts.phi);

    const A_LO: f64 = 0.02;
    const A_HI: f64 = 0.98;
    const P_LO: f64 = 0.60;
    const P_HI: f64 = 0.99;

    let k = if y.len() > 1_000 { 5 } else { 7 };
    let (mut alo, mut ahi) = (A_LO, A_HI);
    let (mut blo, mut bhi) = (A_LO, A_HI);
    let (mut glo, mut ghi) = (A_LO, A_HI);
    let (mut plo, mut phi_hi) = (P_LO, P_HI);
    let mut best: Option<(f64, [f64; 4])> = None;

    for _round in 0..3 {
        let avals = grid(alpha_fixed, alo, ahi, k);
        let bvals = if spec.trend == Trend::None {
            vec![0.0]
        } else {
            grid(beta_fixed, blo, bhi, k)
        };
        let gvals = if spec.seasonal == Seasonal::None {
            vec![0.0]
        } else {
            grid(gamma_fixed, glo, ghi, k)
        };
        let pvals = if spec.trend != Trend::Damped {
            vec![1.0]
        } else {
            grid(phi_fixed, plo, phi_hi, k)
        };
        for &a in &avals {
            for &b in &bvals {
                for &g in &gvals {
                    for &p in &pvals {
                        if let Some(f) = simulate(y, m, spec, a, b, g, p) {
                            if best.map_or(true, |(s, _)| f.sse < s) {
                                best = Some((f.sse, [a, b, g, p]));
                            }
                        }
                    }
                }
            }
        }
        let [ba, bb, bg, bp] = best?.1;
        (alo, ahi) = shrink(ba, alo, ahi, k, A_LO, A_HI);
        (blo, bhi) = shrink(bb, blo, bhi, k, A_LO, A_HI);
        (glo, ghi) = shrink(bg, glo, ghi, k, A_LO, A_HI);
        (plo, phi_hi) = shrink(bp, plo, phi_hi, k, P_LO, P_HI);
    }
    let [a, b, g, p] = best?.1;
    simulate(y, m, spec, a, b, g, p)
}

// ---------------------------------------------------------------- season detection

/// Guess the dominant seasonal cycle length from the autocorrelation of the
/// linearly-detrended series. Returns the shortest lag whose autocorrelation is both strong
/// in absolute terms and close to the strongest lag found, so a quarterly series reports 4
/// rather than its 8- or 12-period harmonics. Advisory only — it never changes the fit.
fn detect_season(y: &[f64]) -> Option<usize> {
    let n = y.len();
    if n < 8 {
        return None;
    }
    let nf = n as f64;
    let mean_t = (nf - 1.0) / 2.0;
    let mean_y = y.iter().sum::<f64>() / nf;
    let (mut sxy, mut sxx) = (0.0f64, 0.0f64);
    for (t, v) in y.iter().enumerate() {
        let dt = t as f64 - mean_t;
        sxy += dt * (v - mean_y);
        sxx += dt * dt;
    }
    let slope = if sxx > 1e-12 { sxy / sxx } else { 0.0 };
    let r: Vec<f64> = y
        .iter()
        .enumerate()
        .map(|(t, v)| v - (mean_y + slope * (t as f64 - mean_t)))
        .collect();
    let denom: f64 = r.iter().map(|v| v * v).sum();
    if !denom.is_finite() || denom < 1e-12 {
        return None;
    }
    let max_lag = (n / 2).min(MAX_SEASON as usize);
    let mut acf: Vec<(usize, f64)> = Vec::new();
    for lag in 2..=max_lag {
        let num: f64 = (lag..n).map(|t| r[t] * r[t - lag]).sum();
        let ac = num / denom;
        if ac.is_finite() {
            acf.push((lag, ac));
        }
    }
    let peak = acf.iter().fold(f64::NEG_INFINITY, |a, (_, c)| a.max(*c));
    if peak < 0.35 {
        return None;
    }
    acf.iter()
        .find(|(_, c)| *c >= 0.35 && *c >= 0.9 * peak)
        .map(|(lag, _)| *lag)
}

// ---------------------------------------------------------------- forecasting

struct Row {
    step: usize,
    period: usize,
    point: f64,
    lower: f64,
    upper: f64,
}

fn z_for(confidence: &str) -> Result<f64, String> {
    match confidence.trim() {
        "80" => Ok(1.281_551_6),
        "90" => Ok(1.644_853_6),
        "95" => Ok(1.959_963_9),
        "99" => Ok(2.575_829_3),
        other => Err(format!(
            "confidence must be one of 80, 90, 95, 99 (got \"{other}\")"
        )),
    }
}

/// `c_j` from the additive-error ETS forecast-variance recursion.
fn c_j(fit: &Fit, j: usize, m: usize) -> f64 {
    let jf = j as f64;
    let mut c = match fit.spec.trend {
        Trend::None => fit.alpha,
        Trend::Additive => fit.alpha + fit.beta * jf,
        Trend::Damped => {
            let p = fit.phi;
            if (1.0 - p).abs() < 1e-9 {
                fit.alpha + fit.beta * jf
            } else {
                fit.alpha + fit.beta * p * (1.0 - p.powi(j as i32)) / (1.0 - p)
            }
        }
    };
    if fit.spec.seasonal != Seasonal::None && m >= 2 && j % m == 0 {
        c += fit.gamma;
    }
    c
}

fn forecast(fit: &Fit, y: &[f64], m: usize, horizon: usize, z: f64) -> Vec<Row> {
    let n = y.len();
    // Relative residual sigma keeps the multiplicative-seasonal bands proportional.
    let rel_sigma = {
        let mut acc = 0.0;
        let mut cnt = 0.0;
        for (t, f) in fit.fitted.iter().enumerate() {
            if let Some(f) = f {
                if f.abs() > 1e-12 {
                    let r = (y[t] - f) / f;
                    acc += r * r;
                    cnt += 1.0;
                }
            }
        }
        if cnt > 0.0 {
            (acc / cnt).sqrt()
        } else {
            0.0
        }
    };
    let sigma = fit.sigma();

    let mut rows = Vec::with_capacity(horizon);
    let mut var_acc = 0.0f64; // sum of c_j^2 for j = 1..h-1
    for h in 1..=horizon {
        if h > 1 {
            let c = c_j(fit, h - 1, m);
            var_acc += c * c;
        }
        let trend_sum = match fit.spec.trend {
            Trend::None => 0.0,
            Trend::Additive => h as f64 * fit.trend,
            Trend::Damped => {
                let p = fit.phi;
                let mut s = 0.0;
                for j in 1..=h {
                    s += p.powi(j as i32);
                }
                s * fit.trend
            }
        };
        let base = fit.level + trend_sum;
        let s = if fit.spec.seasonal != Seasonal::None && m >= 2 {
            let cyc = (h - 1) / m + 1; // ceil(h / m)
            fit.seas[n - 1 + h - m * (cyc - 1)]
        } else {
            0.0
        };
        let point = match fit.spec.seasonal {
            Seasonal::None => base,
            Seasonal::Additive => base + s,
            Seasonal::Multiplicative => base * s,
        };
        let v = (1.0 + var_acc).sqrt();
        let half = if fit.spec.seasonal == Seasonal::Multiplicative {
            z * point.abs() * rel_sigma * v
        } else {
            z * sigma * v
        };
        rows.push(Row {
            step: h,
            period: n + h,
            point,
            lower: point - half,
            upper: point + half,
        });
    }
    rows
}

// ---------------------------------------------------------------- metrics

/// One row of the candidate leaderboard shown when more than one model was fitted.
struct Candidate {
    key: &'static str,
    label: &'static str,
    aicc: f64,
    rmse: f64,
    selected: bool,
}

struct Metrics {
    mae: f64,
    rmse: f64,
    mape: Option<f64>,
    mase: Option<f64>,
}

fn metrics(fit: &Fit, y: &[f64], m: usize) -> Metrics {
    let mut abs_sum = 0.0;
    let mut pct_sum = 0.0;
    let mut pct_cnt = 0.0;
    let mut cnt = 0.0;
    for (t, f) in fit.fitted.iter().enumerate() {
        if let Some(f) = f {
            let e = (y[t] - f).abs();
            abs_sum += e;
            cnt += 1.0;
            if y[t].abs() > 1e-12 {
                pct_sum += e / y[t].abs();
                pct_cnt += 1.0;
            }
        }
    }
    let mae = abs_sum / cnt;
    let lag = if m >= 2 { m } else { 1 };
    let mase = if y.len() > lag {
        let naive: f64 = (lag..y.len()).map(|t| (y[t] - y[t - lag]).abs()).sum::<f64>()
            / (y.len() - lag) as f64;
        if naive > 1e-12 {
            Some(mae / naive)
        } else {
            None
        }
    } else {
        None
    };
    Metrics {
        mae,
        rmse: fit.sigma(),
        mape: (pct_cnt > 0.0).then(|| 100.0 * pct_sum / pct_cnt),
        mase,
    }
}

// ---------------------------------------------------------------- rendering

fn fmt(v: f64, d: usize) -> String {
    let s = format!("{:.*}", d, v);
    if s == format!("-{:.*}", d, 0.0) {
        format!("{:.*}", d, 0.0)
    } else {
        s
    }
}

fn table(headers: &[&str], rows: &[Vec<String>]) -> String {
    let cols = headers.len();
    let mut w: Vec<usize> = headers.iter().map(|h| h.len()).collect();
    for r in rows {
        for c in 0..cols.min(r.len()) {
            w[c] = w[c].max(r[c].len());
        }
    }
    let mut out = String::new();
    let line = |cells: &[String], w: &[usize]| -> String {
        cells
            .iter()
            .enumerate()
            .map(|(i, c)| format!("{:<width$}", c, width = w[i]))
            .collect::<Vec<_>>()
            .join("  ")
            .trim_end()
            .to_string()
    };
    let head: Vec<String> = headers.iter().map(|h| h.to_string()).collect();
    out.push_str(&line(&head, &w));
    out.push('\n');
    for r in rows {
        out.push_str(&line(r, &w));
        out.push('\n');
    }
    out
}

fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

// ---------------------------------------------------------------- entry point

/// Forecast a univariate series. See [`Options`] for the knobs.
pub fn run(data: &str, opts: &Options) -> Result<String, String> {
    let format = opts.format.trim().to_ascii_lowercase();
    if !matches!(format.as_str(), "text" | "csv" | "json") {
        return Err(format!(
            "format must be one of text, csv, json (got \"{format}\")"
        ));
    }
    let z = z_for(&opts.confidence)?;
    if opts.horizon < 1 || opts.horizon > MAX_HORIZON {
        return Err(format!(
            "horizon must be between 1 and {MAX_HORIZON} periods (got {})",
            opts.horizon
        ));
    }
    if opts.season_length < 0 || opts.season_length == 1 || opts.season_length > MAX_SEASON {
        return Err(format!(
            "season_length must be 0 (non-seasonal) or between 2 and {MAX_SEASON} (got {})",
            opts.season_length
        ));
    }
    if opts.decimals < 0 || opts.decimals > 10 {
        return Err(format!(
            "decimals must be between 0 and 10 (got {})",
            opts.decimals
        ));
    }
    for (name, v) in [
        ("alpha", opts.alpha),
        ("beta", opts.beta),
        ("gamma", opts.gamma),
        ("phi", opts.phi),
    ] {
        if !(0.0..=1.0).contains(&v) || !v.is_finite() {
            return Err(format!(
                "{name} must be between 0 and 1 (0 means fit it automatically), got {v}"
            ));
        }
    }
    let d = opts.decimals as usize;
    let horizon = opts.horizon as usize;
    let m = opts.season_length as usize;

    let series = parse_series(data, &opts.header)?;
    let y = &series.values[..];
    let n = y.len();

    let model = opts.model.trim().to_ascii_lowercase();
    let requested: Option<Spec> = SPECS.iter().copied().find(|s| s.key == model);
    if model != "auto" && requested.is_none() {
        return Err(format!(
            "model must be one of auto, simple, holt, damped, holt-winters-additive, holt-winters-multiplicative (got \"{model}\")"
        ));
    }

    let mut notes: Vec<String> = Vec::new();
    let all_positive = y.iter().all(|v| *v > 0.0);

    // Candidate list.
    let candidates: Vec<Spec> = match requested {
        Some(s) => {
            if s.seasonal != Seasonal::None {
                if m < 2 {
                    return Err(format!(
                        "{} needs a season_length of at least 2 (for example 4 for quarterly data or 12 for monthly data)",
                        s.label
                    ));
                }
                if n < 2 * m {
                    return Err(format!(
                        "seasonal models need at least two full cycles: {n} observations with season_length {m} is not enough (need {})",
                        2 * m
                    ));
                }
                if s.seasonal == Seasonal::Multiplicative && !all_positive {
                    return Err(
                        "multiplicative seasonality needs strictly positive values; use holt-winters-additive for series containing zero or negative numbers".into(),
                    );
                }
            } else if m >= 2 {
                notes.push(format!(
                    "season_length {m} was ignored: model \"{}\" has no seasonal component.",
                    s.key
                ));
            }
            vec![s]
        }
        None => {
            let mut v: Vec<Spec> = SPECS
                .iter()
                .copied()
                .filter(|s| s.seasonal == Seasonal::None)
                .collect();
            if m >= 2 {
                if n >= 2 * m {
                    v.push(SPECS[3]);
                    if all_positive {
                        v.push(SPECS[4]);
                    } else {
                        notes.push(
                            "Multiplicative seasonality was skipped: the series contains zero or negative values.".into(),
                        );
                    }
                } else {
                    notes.push(format!(
                        "Seasonal models were skipped: {n} observations is under the two full cycles ({}) that season_length {m} requires.",
                        2 * m
                    ));
                }
            }
            v
        }
    };

    // Fit every candidate, keep the best AICc (ties fall back to SSE).
    let mut fits: Vec<Fit> = Vec::new();
    for spec in candidates {
        if let Some(f) = optimize(y, m, spec, opts) {
            fits.push(f);
        }
    }
    if fits.is_empty() {
        return Err(
            "could not fit a model to this series — check for constant, extremely large, or non-finite values".to_string(),
        );
    }
    let mut best_idx = 0usize;
    for i in 1..fits.len() {
        let (fa, ba) = (fits[i].aicc(m), fits[best_idx].aicc(m));
        let better = if fa.is_finite() && ba.is_finite() {
            fa < ba
        } else if fa.is_finite() {
            true
        } else if ba.is_finite() {
            false
        } else {
            fits[i].sse < fits[best_idx].sse
        };
        if better {
            best_idx = i;
        }
    }
    // Leaderboard of every candidate actually fitted, so an auto pick is auditable.
    let ranking: Vec<Candidate> = fits
        .iter()
        .enumerate()
        .map(|(i, f)| Candidate {
            key: f.spec.key,
            label: f.spec.label,
            aicc: f.aicc(m),
            rmse: f.sigma(),
            selected: i == best_idx,
        })
        .collect();
    let fit = fits.swap_remove(best_idx);

    let met = metrics(&fit, y, m);
    let rows = forecast(&fit, y, m, horizon, z);
    let seasonal_on = fit.spec.seasonal != Seasonal::None;
    let aicc = fit.aicc(if seasonal_on { m } else { 0 });

    if met.mape.is_none() {
        notes.push("MAPE is not reported: every fitted observation is zero.".into());
    } else if y.iter().any(|v| v.abs() < 1e-12) {
        notes.push("MAPE skips observations equal to zero.".into());
    }
    if fit.spec.seasonal == Seasonal::Multiplicative {
        notes.push(
            "Multiplicative-seasonal prediction intervals have no closed form; these bands scale a relative residual sigma by the point forecast and are approximate.".into(),
        );
    }
    if model == "auto" {
        notes.push(format!(
            "Model chosen automatically by AICc from {} candidates.",
            if m >= 2 && n >= 2 * m {
                if all_positive {
                    "5"
                } else {
                    "4"
                }
            } else {
                "3"
            }
        ));
    }
    if !seasonal_on {
        if let Some(lag) = detect_season(y) {
            if n >= 2 * lag {
                notes.push(format!(
                    "A repeating cycle of about {lag} periods shows up in this series. Set season_length to {lag} to model it with Holt-Winters — this run treated the series as non-seasonal."
                ));
            }
        }
    }
    notes.push(
        "Prediction intervals assume the fitted model is correct and its errors are independent and normally distributed; they widen with the horizon but cannot cover structural change.".into(),
    );

    let conf = opts.confidence.trim();
    match format.as_str() {
        "json" => {
            let fc: Vec<serde_json::Value> = rows
                .iter()
                .map(|r| {
                    json!({
                        "step": r.step,
                        "period": r.period,
                        "forecast": round(r.point, d),
                        "lower": round(r.lower, d),
                        "upper": round(r.upper, d),
                    })
                })
                .collect();
            let mut root = json!({
                "model": fit.spec.key,
                "model_label": fit.spec.label,
                "observations": n,
                "season_length": if seasonal_on { m } else { 0 },
                "horizon": horizon,
                "confidence": conf,
                "parameters": {
                    "alpha": round(fit.alpha, 4),
                    "beta": if fit.spec.trend == Trend::None { serde_json::Value::Null } else { json!(round(fit.beta, 4)) },
                    "gamma": if seasonal_on { json!(round(fit.gamma, 4)) } else { serde_json::Value::Null },
                    "phi": if fit.spec.trend == Trend::Damped { json!(round(fit.phi, 4)) } else { serde_json::Value::Null },
                },
                "accuracy": {
                    "mae": round(met.mae, d),
                    "rmse": round(met.rmse, d),
                    "mape_percent": met.mape.map(|v| round(v, d)),
                    "mase": met.mase.map(|v| round(v, d)),
                    "sigma": round(fit.sigma(), d),
                    "aicc": if aicc.is_finite() { json!(round(aicc, d)) } else { serde_json::Value::Null },
                    "fitted_points": fit.n_fit,
                },
                "forecast": fc,
                "notes": notes,
            });
            if ranking.len() > 1 {
                root["candidates"] = json!(ranking
                    .iter()
                    .map(|c| json!({
                        "model": c.key,
                        "model_label": c.label,
                        "aicc": if c.aicc.is_finite() { json!(round(c.aicc, d)) } else { serde_json::Value::Null },
                        "rmse": round(c.rmse, d),
                        "selected": c.selected,
                    }))
                    .collect::<Vec<_>>());
            }
            if opts.show_fitted {
                let f: Vec<serde_json::Value> = (0..n)
                    .map(|t| {
                        json!({
                            "period": t + 1,
                            "label": series.labels[t],
                            "actual": round(y[t], d),
                            "fitted": fit.fitted[t].map(|v| round(v, d)),
                            "residual": fit.fitted[t].map(|v| round(y[t] - v, d)),
                        })
                    })
                    .collect();
                root["fitted"] = json!(f);
            }
            serde_json::to_string_pretty(&root).map_err(|e| e.to_string())
        }
        "csv" => {
            let mut out = String::new();
            out.push_str("section,key,value\n");
            for (k, v) in [
                ("model", fit.spec.key.to_string()),
                ("model_label", fit.spec.label.to_string()),
                ("observations", n.to_string()),
                (
                    "season_length",
                    if seasonal_on { m.to_string() } else { "0".into() },
                ),
                ("horizon", horizon.to_string()),
                ("confidence_percent", conf.to_string()),
                ("alpha", fmt(fit.alpha, 4)),
                (
                    "beta",
                    if fit.spec.trend == Trend::None {
                        String::new()
                    } else {
                        fmt(fit.beta, 4)
                    },
                ),
                (
                    "gamma",
                    if seasonal_on {
                        fmt(fit.gamma, 4)
                    } else {
                        String::new()
                    },
                ),
                (
                    "phi",
                    if fit.spec.trend == Trend::Damped {
                        fmt(fit.phi, 4)
                    } else {
                        String::new()
                    },
                ),
                ("mae", fmt(met.mae, d)),
                ("rmse", fmt(met.rmse, d)),
                (
                    "mape_percent",
                    met.mape.map(|v| fmt(v, d)).unwrap_or_default(),
                ),
                ("mase", met.mase.map(|v| fmt(v, d)).unwrap_or_default()),
                ("sigma", fmt(fit.sigma(), d)),
                (
                    "aicc",
                    if aicc.is_finite() {
                        fmt(aicc, d)
                    } else {
                        String::new()
                    },
                ),
            ] {
                out.push_str(&format!("summary,{},{}\n", k, csv_escape(&v)));
            }
            if ranking.len() > 1 {
                out.push('\n');
                out.push_str("candidate,model,aicc,rmse,selected\n");
                for c in &ranking {
                    out.push_str(&format!(
                        "candidate,{},{},{},{}\n",
                        c.key,
                        if c.aicc.is_finite() {
                            fmt(c.aicc, d)
                        } else {
                            String::new()
                        },
                        fmt(c.rmse, d),
                        if c.selected { "yes" } else { "no" }
                    ));
                }
            }
            out.push('\n');
            out.push_str(&format!("step,period,forecast,lower_{conf},upper_{conf}\n"));
            for r in &rows {
                out.push_str(&format!(
                    "{},{},{},{},{}\n",
                    r.step,
                    r.period,
                    fmt(r.point, d),
                    fmt(r.lower, d),
                    fmt(r.upper, d)
                ));
            }
            if opts.show_fitted {
                out.push('\n');
                out.push_str("period,label,actual,fitted,residual\n");
                for t in 0..n {
                    out.push_str(&format!(
                        "{},{},{},{},{}\n",
                        t + 1,
                        csv_escape(&series.labels[t]),
                        fmt(y[t], d),
                        fit.fitted[t].map(|v| fmt(v, d)).unwrap_or_default(),
                        fit.fitted[t].map(|v| fmt(y[t] - v, d)).unwrap_or_default()
                    ));
                }
            }
            Ok(out.trim_end().to_string())
        }
        _ => {
            let mut out = String::new();
            out.push_str("Time series forecast\n");
            out.push_str(&format!(
                "Model: {} ({}){}\n",
                fit.spec.label,
                fit.spec.key,
                if model == "auto" {
                    " — auto-selected"
                } else {
                    ""
                }
            ));
            out.push_str(&format!(
                "Observations: {}   Season length: {}   Horizon: {}   Interval: {}%\n",
                n,
                if seasonal_on {
                    m.to_string()
                } else {
                    "none".to_string()
                },
                horizon,
                conf
            ));

            out.push_str("\nSmoothing parameters\n");
            let mut prm: Vec<Vec<String>> = vec![vec![
                "alpha (level)".into(),
                fmt(fit.alpha, 4),
            ]];
            if fit.spec.trend != Trend::None {
                prm.push(vec!["beta (trend)".into(), fmt(fit.beta, 4)]);
            }
            if fit.spec.trend == Trend::Damped {
                prm.push(vec!["phi (damping)".into(), fmt(fit.phi, 4)]);
            }
            if seasonal_on {
                prm.push(vec!["gamma (season)".into(), fmt(fit.gamma, 4)]);
            }
            out.push_str(&table(&["parameter", "value"], &prm));

            out.push_str("\nIn-sample one-step-ahead accuracy\n");
            let mut acc: Vec<Vec<String>> = vec![
                vec!["MAE".into(), fmt(met.mae, d)],
                vec!["RMSE".into(), fmt(met.rmse, d)],
            ];
            if let Some(v) = met.mape {
                acc.push(vec!["MAPE".into(), format!("{}%", fmt(v, d))]);
            }
            if let Some(v) = met.mase {
                acc.push(vec!["MASE".into(), fmt(v, d)]);
            }
            acc.push(vec!["Residual sigma".into(), fmt(fit.sigma(), d)]);
            if aicc.is_finite() {
                acc.push(vec!["AICc".into(), fmt(aicc, d)]);
            }
            acc.push(vec!["Fitted points".into(), fit.n_fit.to_string()]);
            out.push_str(&table(&["metric", "value"], &acc));

            if ranking.len() > 1 {
                out.push_str("\nModel comparison (lower AICc is better)\n");
                let crows: Vec<Vec<String>> = ranking
                    .iter()
                    .map(|c| {
                        vec![
                            c.key.to_string(),
                            if c.aicc.is_finite() {
                                fmt(c.aicc, d)
                            } else {
                                "—".into()
                            },
                            fmt(c.rmse, d),
                            if c.selected { "chosen".into() } else { String::new() },
                        ]
                    })
                    .collect();
                out.push_str(&table(&["model", "AICc", "RMSE", ""], &crows));
            }

            out.push_str(&format!("\nForecast ({conf}% prediction interval)\n"));
            let frows: Vec<Vec<String>> = rows
                .iter()
                .map(|r| {
                    vec![
                        r.step.to_string(),
                        r.period.to_string(),
                        fmt(r.point, d),
                        fmt(r.lower, d),
                        fmt(r.upper, d),
                    ]
                })
                .collect();
            out.push_str(&table(
                &["step", "period", "forecast", "lower", "upper"],
                &frows,
            ));

            if opts.show_fitted {
                out.push_str("\nFitted values and residuals\n");
                let use_label = series.has_labels;
                let frs: Vec<Vec<String>> = (0..n)
                    .map(|t| {
                        let mut row = vec![(t + 1).to_string()];
                        if use_label {
                            row.push(series.labels[t].clone());
                        }
                        row.push(fmt(y[t], d));
                        row.push(
                            fit.fitted[t]
                                .map(|v| fmt(v, d))
                                .unwrap_or_else(|| "—".into()),
                        );
                        row.push(
                            fit.fitted[t]
                                .map(|v| fmt(y[t] - v, d))
                                .unwrap_or_else(|| "—".into()),
                        );
                        row
                    })
                    .collect();
                let headers: Vec<&str> = if use_label {
                    vec!["period", "label", "actual", "fitted", "residual"]
                } else {
                    vec!["period", "actual", "fitted", "residual"]
                };
                out.push_str(&table(&headers, &frs));
            }

            out.push_str("\nNotes\n");
            for note in &notes {
                out.push_str(&format!("- {note}\n"));
            }
            Ok(out.trim_end().to_string())
        }
    }
}

fn round(v: f64, d: usize) -> f64 {
    let f = 10f64.powi(d as i32);
    if !v.is_finite() {
        return v;
    }
    (v * f).round() / f
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> Options {
        Options::default()
    }

    #[test]
    fn perfect_linear_trend_is_extrapolated() {
        // y = 2t: Holt should continue the line almost exactly.
        let data = (1..=20)
            .map(|t| (2 * t).to_string())
            .collect::<Vec<_>>()
            .join("\n");
        let mut o = opts();
        o.model = "holt".into();
        o.horizon = 3;
        o.format = "json".into();
        let out = run(&data, &o).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        let f0 = v["forecast"][0]["forecast"].as_f64().unwrap();
        let f2 = v["forecast"][2]["forecast"].as_f64().unwrap();
        assert!((f0 - 42.0).abs() < 0.5, "expected ~42, got {f0}");
        assert!((f2 - 46.0).abs() < 0.5, "expected ~46, got {f2}");
        assert_eq!(v["model"], "holt");
        assert_eq!(v["forecast"][0]["period"], 21);
    }

    #[test]
    fn constant_series_forecasts_the_constant() {
        let data = "5\n5\n5\n5\n5\n5";
        let mut o = opts();
        o.model = "simple".into();
        o.horizon = 2;
        let out = run(data, &o).unwrap();
        assert!(out.contains("Simple exponential smoothing"), "{out}");
        assert!(out.contains("5.000"), "{out}");
    }

    #[test]
    fn seasonal_series_recovers_the_cycle() {
        // Quarterly pattern +10/-10 around a rising level.
        let mut vals = Vec::new();
        for k in 0..6 {
            let base = 100.0 + 4.0 * k as f64;
            vals.push(base + 10.0);
            vals.push(base);
            vals.push(base - 10.0);
            vals.push(base);
        }
        let data = vals
            .iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        let mut o = opts();
        o.model = "holt-winters-additive".into();
        o.season_length = 4;
        o.horizon = 4;
        o.format = "json".into();
        let out = run(&data, &o).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        let f: Vec<f64> = (0..4)
            .map(|i| v["forecast"][i]["forecast"].as_f64().unwrap())
            .collect();
        // Next cycle continues the +10 / 0 / -10 / 0 shape.
        assert!(f[0] > f[1] && f[1] > f[2], "peak-then-trough expected: {f:?}");
        assert!(f[3] > f[2], "recovery expected: {f:?}");
        assert!((f[0] - f[2] - 20.0).abs() < 6.0, "amplitude ~20: {f:?}");
    }

    #[test]
    fn labels_and_header_row_are_accepted() {
        let data = "month,sales\nJan,10\nFeb,12\nMar,14\nApr,16";
        let mut o = opts();
        o.model = "holt".into();
        o.horizon = 1;
        o.show_fitted = true;
        let out = run(data, &o).unwrap();
        assert!(out.contains("Observations: 4"), "{out}");
        assert!(out.contains("Jan"), "{out}");
    }

    #[test]
    fn single_row_csv_series_parses() {
        let mut o = opts();
        o.model = "simple".into();
        o.horizon = 1;
        let out = run("10, 11, 12, 13, 14", &o).unwrap();
        assert!(out.contains("Observations: 5"), "{out}");
    }

    #[test]
    fn interval_widens_with_horizon() {
        let data = "10\n12\n11\n13\n12\n14\n13\n15\n14\n16";
        let mut o = opts();
        o.model = "simple".into();
        o.horizon = 5;
        o.format = "json".into();
        let v: serde_json::Value = serde_json::from_str(&run(data, &o).unwrap()).unwrap();
        let w1 = v["forecast"][0]["upper"].as_f64().unwrap()
            - v["forecast"][0]["lower"].as_f64().unwrap();
        let w5 = v["forecast"][4]["upper"].as_f64().unwrap()
            - v["forecast"][4]["lower"].as_f64().unwrap();
        assert!(w5 > w1, "bands must widen: {w1} -> {w5}");
    }

    #[test]
    fn too_few_points_is_an_error() {
        let err = run("1\n2", &opts()).unwrap_err();
        assert!(err.contains("at least 3 observations"), "{err}");
    }

    #[test]
    fn non_numeric_value_is_an_error() {
        let err = run("10\n11\nabc\n13", &opts()).unwrap_err();
        assert!(err.contains("is not a number"), "{err}");
    }

    #[test]
    fn seasonal_model_without_season_length_is_an_error() {
        let mut o = opts();
        o.model = "holt-winters-additive".into();
        let err = run("1\n2\n3\n4\n5\n6\n7\n8", &o).unwrap_err();
        assert!(err.contains("season_length"), "{err}");
    }

    #[test]
    fn seasonal_model_needs_two_full_cycles() {
        let mut o = opts();
        o.model = "holt-winters-additive".into();
        o.season_length = 12;
        let err = run("1\n2\n3\n4\n5\n6\n7\n8", &o).unwrap_err();
        assert!(err.contains("two full cycles"), "{err}");
    }

    #[test]
    fn multiplicative_rejects_non_positive_values() {
        let mut o = opts();
        o.model = "holt-winters-multiplicative".into();
        o.season_length = 2;
        let err = run("1\n0\n3\n4\n5\n6\n7\n8", &o).unwrap_err();
        assert!(err.contains("strictly positive"), "{err}");
    }

    #[test]
    fn bad_enum_values_are_rejected() {
        let mut o = opts();
        o.format = "yaml".into();
        assert!(run("1\n2\n3", &o).unwrap_err().contains("format must be"));
        let mut o = opts();
        o.confidence = "50".into();
        assert!(run("1\n2\n3", &o)
            .unwrap_err()
            .contains("confidence must be"));
        let mut o = opts();
        o.model = "arima".into();
        assert!(run("1\n2\n3", &o).unwrap_err().contains("model must be"));
        let mut o = opts();
        o.horizon = 0;
        assert!(run("1\n2\n3", &o).unwrap_err().contains("horizon must be"));
        let mut o = opts();
        o.alpha = 1.5;
        assert!(run("1\n2\n3", &o).unwrap_err().contains("alpha must be"));
    }

    #[test]
    fn fixed_alpha_is_respected() {
        let mut o = opts();
        o.model = "simple".into();
        o.alpha = 0.5;
        o.format = "json".into();
        let v: serde_json::Value = serde_json::from_str(&run("1\n3\n2\n4\n3\n5", &o).unwrap()).unwrap();
        assert_eq!(v["parameters"]["alpha"].as_f64().unwrap(), 0.5);
    }

    #[test]
    fn csv_format_has_summary_and_forecast_sections() {
        let mut o = opts();
        o.model = "simple".into();
        o.horizon = 2;
        o.format = "csv".into();
        let out = run("4\n5\n6\n7", &o).unwrap();
        assert!(out.starts_with("section,key,value\n"), "{out}");
        assert!(out.contains("step,period,forecast,lower_95,upper_95"), "{out}");
    }

    #[test]
    fn cap_boundary_is_enforced() {
        let ok = (0..MAX_POINTS).map(|i| (i % 7).to_string()).collect::<Vec<_>>().join("\n");
        let mut o = opts();
        o.model = "simple".into();
        o.horizon = 1;
        assert!(run(&ok, &o).is_ok());
        let too_many = format!("{ok}\n1");
        let err = run(&too_many, &o).unwrap_err();
        assert!(err.contains("too many observations"), "{err}");
    }

    #[test]
    fn auto_reports_a_candidate_leaderboard_with_one_winner() {
        let data = (1..=24)
            .map(|t| (100 + 5 * t).to_string())
            .collect::<Vec<_>>()
            .join("\n");
        let mut o = opts();
        o.horizon = 2;
        o.format = "json".into();
        let v: serde_json::Value = serde_json::from_str(&run(&data, &o).unwrap()).unwrap();
        let cands = v["candidates"].as_array().unwrap();
        assert_eq!(cands.len(), 3, "3 non-seasonal candidates: {cands:?}");
        let chosen: Vec<&str> = cands
            .iter()
            .filter(|c| c["selected"] == true)
            .map(|c| c["model"].as_str().unwrap())
            .collect();
        assert_eq!(chosen.len(), 1, "exactly one winner: {cands:?}");
        assert_eq!(chosen[0], v["model"].as_str().unwrap());

        // A pinned model fits a single candidate, so there is nothing to compare.
        let mut o2 = opts();
        o2.model = "simple".into();
        o2.format = "json".into();
        let v2: serde_json::Value = serde_json::from_str(&run(&data, &o2).unwrap()).unwrap();
        assert!(v2.get("candidates").is_none(), "{v2}");

        // Text and CSV surface the same table.
        let mut o3 = opts();
        o3.horizon = 2;
        let text = run(&data, &o3).unwrap();
        assert!(text.contains("Model comparison"), "{text}");
        assert!(text.contains("chosen"), "{text}");
        let mut o4 = opts();
        o4.horizon = 2;
        o4.format = "csv".into();
        let csv = run(&data, &o4).unwrap();
        assert!(csv.contains("candidate,model,aicc,rmse,selected"), "{csv}");
    }

    #[test]
    fn non_seasonal_run_hints_at_a_detected_cycle() {
        // Quarterly +10/0/-10/0 shape on a rising level, run WITHOUT season_length.
        let mut vals = Vec::new();
        for k in 0..6 {
            let base = 100.0 + 4.0 * k as f64;
            vals.extend_from_slice(&[base + 10.0, base, base - 10.0, base]);
        }
        let data = vals
            .iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        let mut o = opts();
        o.horizon = 2;
        let out = run(&data, &o).unwrap();
        assert!(
            out.contains("cycle of about 4 periods"),
            "expected a season_length hint: {out}"
        );
        // A flat trending series has no cycle to report.
        let plain = (1..=24)
            .map(|t| (100 + 5 * t).to_string())
            .collect::<Vec<_>>()
            .join("\n");
        let out2 = run(&plain, &o).unwrap();
        assert!(!out2.contains("cycle of about"), "{out2}");
        // Neither does a run that already models the season.
        let mut o2 = opts();
        o2.model = "holt-winters-additive".into();
        o2.season_length = 4;
        o2.horizon = 2;
        let out3 = run(&data, &o2).unwrap();
        assert!(!out3.contains("cycle of about"), "{out3}");
    }

    #[test]
    fn auto_prefers_a_trend_model_on_trending_data() {
        let data = (1..=24)
            .map(|t| (100 + 5 * t).to_string())
            .collect::<Vec<_>>()
            .join("\n");
        let mut o = opts();
        o.horizon = 2;
        o.format = "json".into();
        let v: serde_json::Value = serde_json::from_str(&run(&data, &o).unwrap()).unwrap();
        let model = v["model"].as_str().unwrap().to_string();
        assert!(model == "holt" || model == "damped", "got {model}");
        let f0 = v["forecast"][0]["forecast"].as_f64().unwrap();
        assert!(f0 > 220.0, "should extrapolate upward, got {f0}");
    }
}
