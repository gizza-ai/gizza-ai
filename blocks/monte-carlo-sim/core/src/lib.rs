//! gizza-ai/monte-carlo-sim core — pure Monte Carlo simulation of a user-defined
//! model, shared by the chat skill block and the standalone web page. No
//! wafer/wasm-bindgen deps.
//!
//! You declare uncertain input variables as probability distributions
//! (`revenue = normal(1000, 200)`), combine them in a `model` expression
//! (`revenue - cost`), and this samples `trials` outcomes and reports the
//! outcome distribution: mean/std-dev/min/max, percentiles, an optional
//! probability of meeting a target, and a text histogram.
//!
//! Randomness is a seeded, hand-rolled SplitMix64 PRNG — fully deterministic
//! given the same inputs + `seed`, and free of `getrandom` so it instantiates on
//! every backend (wasm32-wasip1 chat/CLI and wasm32-unknown-unknown page).

use meval::Expr;
use serde::Serialize;

/// Hard cap on trials so a runaway request can't hang the browser tab.
pub const MAX_TRIALS: i64 = 1_000_000;
/// Minimum trials — below this the percentiles are meaningless noise.
pub const MIN_TRIALS: i64 = 100;
/// Cap on histogram rows (0 = hide the histogram).
pub const MAX_BINS: i64 = 40;

/// Deterministic SplitMix64 PRNG. Seeded from the `seed` param; produces the
/// same stream on every platform (no `getrandom`).
struct Rng {
    state: u64,
}

impl Rng {
    fn new(seed: u64) -> Self {
        Rng { state: seed }
    }
    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    /// Uniform in [0, 1).
    fn uniform01(&mut self) -> f64 {
        // Top 53 bits → exact f64 mantissa, standard construction.
        (self.next_u64() >> 11) as f64 * (1.0 / ((1u64 << 53) as f64))
    }
    /// Standard normal via Box–Muller (uses two uniforms per draw).
    fn standard_normal(&mut self) -> f64 {
        // Guard u1 away from 0 so ln() is finite.
        let mut u1 = self.uniform01();
        if u1 <= f64::MIN_POSITIVE {
            u1 = f64::MIN_POSITIVE;
        }
        let u2 = self.uniform01();
        (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos()
    }
}

/// A parsed input variable with its distribution.
#[derive(Debug, Clone, PartialEq)]
struct Variable {
    name: String,
    dist: Dist,
}

#[derive(Debug, Clone, PartialEq)]
enum Dist {
    Constant(f64),
    /// `uniform(min, max)`
    Uniform(f64, f64),
    /// `normal(mean, std_dev)`
    Normal(f64, f64),
    /// `lognormal(mu, sigma)` — ln(value) ~ Normal(mu, sigma).
    Lognormal(f64, f64),
    /// `triangular(min, mode, max)`
    Triangular(f64, f64, f64),
}

impl Dist {
    fn sample(&self, rng: &mut Rng) -> f64 {
        match *self {
            Dist::Constant(v) => v,
            Dist::Uniform(a, b) => a + rng.uniform01() * (b - a),
            Dist::Normal(mean, sd) => mean + sd * rng.standard_normal(),
            Dist::Lognormal(mu, sigma) => (mu + sigma * rng.standard_normal()).exp(),
            Dist::Triangular(min, mode, max) => {
                let u = rng.uniform01();
                let fc = (mode - min) / (max - min);
                if u < fc {
                    min + (u * (max - min) * (mode - min)).sqrt()
                } else {
                    max - ((1.0 - u) * (max - min) * (max - mode)).sqrt()
                }
            }
        }
    }
}

/// The reported outcome distribution.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SimResult {
    pub trials: usize,
    pub seed: u64,
    pub mean: f64,
    pub std_dev: f64,
    pub min: f64,
    pub max: f64,
    /// Percentile values at 5/10/25/50/75/90/95/99.
    pub p5: f64,
    pub p10: f64,
    pub p25: f64,
    pub p50: f64,
    pub p75: f64,
    pub p90: f64,
    pub p95: f64,
    pub p99: f64,
    /// The target and comparison, echoed back when a threshold is given.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub threshold: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub threshold_direction: Option<String>,
    /// Fraction (0..=1) of outcomes meeting the threshold.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub threshold_probability: Option<f64>,
}

fn round(v: f64, places: i32) -> f64 {
    if !v.is_finite() {
        return v;
    }
    let f = 10f64.powi(places);
    (v * f).round() / f
}

/// Linear-interpolation percentile (numpy default), `p` in 0.0..=1.0, on a
/// sorted slice.
fn percentile(sorted: &[f64], p: f64) -> f64 {
    let n = sorted.len();
    if n == 1 {
        return sorted[0];
    }
    let rank = p * (n as f64 - 1.0);
    let lo = rank.floor() as usize;
    let hi = rank.ceil() as usize;
    if lo == hi {
        sorted[lo]
    } else {
        let frac = rank - lo as f64;
        sorted[lo] + (sorted[hi] - sorted[lo]) * frac
    }
}

/// Parse one `min, max, ...` argument list into floats.
fn parse_args(inner: &str) -> Result<Vec<f64>, String> {
    let mut out = Vec::new();
    for tok in inner.split(',') {
        let t = tok.trim();
        if t.is_empty() {
            return Err("a distribution argument is empty (check for a stray comma)".into());
        }
        let v: f64 = t
            .parse()
            .map_err(|_| format!("'{t}' is not a number in the distribution arguments"))?;
        if !v.is_finite() {
            return Err(format!("distribution argument '{t}' is not finite"));
        }
        out.push(v);
    }
    Ok(out)
}

fn parse_dist(spec: &str) -> Result<Dist, String> {
    let spec = spec.trim();
    let open = spec.find('(').ok_or_else(|| {
        format!("distribution '{spec}' must look like name(args), e.g. normal(0, 1)")
    })?;
    if !spec.ends_with(')') {
        return Err(format!("distribution '{spec}' is missing its closing ')'"));
    }
    let name = spec[..open].trim().to_ascii_lowercase();
    let inner = &spec[open + 1..spec.len() - 1];
    let a = parse_args(inner)?;
    let want = |n: usize| -> Result<(), String> {
        if a.len() == n {
            Ok(())
        } else {
            Err(format!("{name}(...) takes {n} argument(s), got {}", a.len()))
        }
    };
    match name.as_str() {
        "constant" | "const" | "fixed" => {
            want(1)?;
            Ok(Dist::Constant(a[0]))
        }
        "uniform" => {
            want(2)?;
            if a[0] >= a[1] {
                return Err("uniform(min, max) needs min < max".into());
            }
            Ok(Dist::Uniform(a[0], a[1]))
        }
        "normal" | "gaussian" => {
            want(2)?;
            if a[1] <= 0.0 {
                return Err("normal(mean, std_dev) needs std_dev > 0".into());
            }
            Ok(Dist::Normal(a[0], a[1]))
        }
        "lognormal" => {
            want(2)?;
            if a[1] <= 0.0 {
                return Err("lognormal(mu, sigma) needs sigma > 0".into());
            }
            Ok(Dist::Lognormal(a[0], a[1]))
        }
        "triangular" | "triangle" => {
            want(3)?;
            let (min, mode, max) = (a[0], a[1], a[2]);
            if !(min < max && (min..=max).contains(&mode)) {
                return Err(
                    "triangular(min, mode, max) needs min < max and min <= mode <= max".into(),
                );
            }
            Ok(Dist::Triangular(min, mode, max))
        }
        other => Err(format!(
            "unknown distribution '{other}'. Supported: constant, uniform, normal, lognormal, triangular"
        )),
    }
}

fn is_ident(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Parse the `variables` block: one `name = dist(args)` per line. Blank lines and
/// `#`-prefixed comment lines are ignored.
fn parse_variables(text: &str) -> Result<Vec<Variable>, String> {
    let mut vars: Vec<Variable> = Vec::new();
    for (i, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let eq = line
            .find('=')
            .ok_or_else(|| format!("line {}: expected 'name = distribution(...)'", i + 1))?;
        let name = line[..eq].trim().to_string();
        if !is_ident(&name) {
            return Err(format!(
                "line {}: '{name}' is not a valid variable name (letters, digits, underscore; must not start with a digit)",
                i + 1
            ));
        }
        if vars.iter().any(|v| v.name == name) {
            return Err(format!("variable '{name}' is defined more than once"));
        }
        let dist = parse_dist(&line[eq + 1..]).map_err(|e| format!("line {}: {e}", i + 1))?;
        vars.push(Variable { name, dist });
    }
    if vars.is_empty() {
        return Err("no input variables — declare at least one, e.g. 'x = normal(0, 1)'".into());
    }
    Ok(vars)
}

/// One simulation pass: returns the SORTED outcomes plus mean & population
/// std-dev. Shared by `simulate` (stats) and `report` (histogram).
fn run_raw(variables: &str, model: &str, trials: i64, seed: u64) -> Result<(Vec<f64>, f64, f64), String> {
    if !(MIN_TRIALS..=MAX_TRIALS).contains(&trials) {
        return Err(format!("trials must be between {MIN_TRIALS} and {MAX_TRIALS}"));
    }
    let model = model.trim();
    if model.is_empty() {
        return Err("the model expression is empty".into());
    }
    let vars = parse_variables(variables)?;

    // Compile the model once; bind every declared variable by name. meval errors
    // if the model references an unbound name.
    let expr: Expr = model
        .parse()
        .map_err(|e| format!("could not parse the model '{model}': {e}"))?;
    let names: Vec<&str> = vars.iter().map(|v| v.name.as_str()).collect();
    let func = expr
        .bindn(&names)
        .map_err(|e| format!("model references an unknown variable: {e}"))?;

    let mut rng = Rng::new(seed);
    let n = trials as usize;
    let mut outcomes: Vec<f64> = Vec::with_capacity(n);
    let mut sample = vec![0.0f64; vars.len()];
    let mut total = 0.0f64;
    for _ in 0..n {
        for (j, v) in vars.iter().enumerate() {
            sample[j] = v.dist.sample(&mut rng);
        }
        let y = func(&sample);
        if !y.is_finite() {
            return Err("the model produced a non-finite value (NaN/inf) — check for division by zero or log of a negative sample".into());
        }
        total += y;
        outcomes.push(y);
    }

    let mean = total / n as f64;
    let var = outcomes.iter().map(|y| (y - mean).powi(2)).sum::<f64>() / n as f64;
    outcomes.sort_by(|a, b| a.partial_cmp(b).unwrap());
    Ok((outcomes, mean, var.sqrt()))
}

fn resolve_direction(direction: &str) -> Result<&str, String> {
    let direction = if direction.trim().is_empty() {
        "at-or-above"
    } else {
        direction.trim()
    };
    if !matches!(direction, "at-or-above" | "at-or-below") {
        return Err("threshold_direction must be 'at-or-above' or 'at-or-below'".into());
    }
    Ok(direction)
}

/// Run the simulation and return the structured result. `direction` is
/// "at-or-above" or "at-or-below" (used only when `threshold` is `Some`).
pub fn simulate(
    variables: &str,
    model: &str,
    trials: i64,
    seed: u64,
    threshold: Option<f64>,
    direction: &str,
) -> Result<SimResult, String> {
    let direction = resolve_direction(direction)?;
    let (sorted, mean, std_dev) = run_raw(variables, model, trials, seed)?;
    let n = sorted.len();

    let (threshold_out, dir_out, prob) = if let Some(t) = threshold {
        let hits = match direction {
            "at-or-below" => sorted.iter().filter(|&&y| y <= t).count(),
            _ => sorted.iter().filter(|&&y| y >= t).count(),
        };
        (
            Some(t),
            Some(direction.to_string()),
            Some(round(hits as f64 / n as f64, 6)),
        )
    } else {
        (None, None, None)
    };

    Ok(SimResult {
        trials: n,
        seed,
        mean: round(mean, 6),
        std_dev: round(std_dev, 6),
        min: round(sorted[0], 6),
        max: round(sorted[n - 1], 6),
        p5: round(percentile(&sorted, 0.05), 6),
        p10: round(percentile(&sorted, 0.10), 6),
        p25: round(percentile(&sorted, 0.25), 6),
        p50: round(percentile(&sorted, 0.50), 6),
        p75: round(percentile(&sorted, 0.75), 6),
        p90: round(percentile(&sorted, 0.90), 6),
        p95: round(percentile(&sorted, 0.95), 6),
        p99: round(percentile(&sorted, 0.99), 6),
        threshold: threshold_out,
        threshold_direction: dir_out,
        threshold_probability: prob,
    })
}

/// Trim a rounded f64 to a compact string (no trailing `.0` for whole numbers, at
/// most 4 decimals otherwise).
fn fmt(v: f64) -> String {
    let r = round(v, 4);
    if r == r.trunc() && r.abs() < 1e15 {
        format!("{}", r as i64)
    } else {
        let s = format!("{r:.4}");
        let s = s.trim_end_matches('0');
        let s = s.trim_end_matches('.');
        s.to_string()
    }
}

/// Build the text histogram over the sorted outcomes with `bins` rows.
fn histogram(sorted: &[f64], bins: usize) -> String {
    let lo = sorted[0];
    let hi = sorted[sorted.len() - 1];
    if bins == 0 || hi <= lo {
        return String::new();
    }
    let width = (hi - lo) / bins as f64;
    let mut counts = vec![0usize; bins];
    for &y in sorted {
        let mut idx = ((y - lo) / width) as usize;
        if idx >= bins {
            idx = bins - 1;
        }
        counts[idx] += 1;
    }
    let peak = counts.iter().copied().max().unwrap_or(1).max(1);
    let mut lines = vec!["Histogram:".to_string()];
    for (i, &c) in counts.iter().enumerate() {
        let start = lo + width * i as f64;
        let end = start + width;
        let bar_len = (c * 40 + peak / 2) / peak; // scaled 0..40, rounded
        let bar: String = "#".repeat(bar_len);
        lines.push(format!("  {:>12} .. {:<12} | {} {}", fmt(start), fmt(end), bar, c));
    }
    lines.join("\n")
}

/// Human-readable report (used by the page + chat). One simulation pass; formats
/// the distribution, percentiles, optional threshold probability, and a
/// histogram (`bins` rows; 0 hides it).
#[allow(clippy::too_many_arguments)]
pub fn report(
    variables: &str,
    model: &str,
    trials: i64,
    seed: u64,
    threshold: Option<f64>,
    direction: &str,
    bins: i64,
) -> Result<String, String> {
    if !(0..=MAX_BINS).contains(&bins) {
        return Err(format!("histogram_bins must be between 0 and {MAX_BINS}"));
    }
    let direction = resolve_direction(direction)?;
    let (sorted, mean, std_dev) = run_raw(variables, model, trials, seed)?;
    let n = sorted.len();

    let mut out = String::new();
    out.push_str(&format!(
        "Monte Carlo simulation — {n} trials (seed {seed})\n\n"
    ));
    out.push_str("Outcome distribution:\n");
    out.push_str(&format!("  mean    : {}\n", fmt(round(mean, 6))));
    out.push_str(&format!("  std dev : {}\n", fmt(round(std_dev, 6))));
    out.push_str(&format!("  min     : {}\n", fmt(round(sorted[0], 6))));
    out.push_str(&format!("  max     : {}\n\n", fmt(round(sorted[n - 1], 6))));
    out.push_str("Percentiles:\n");
    out.push_str(&format!("  P5      : {}\n", fmt(round(percentile(&sorted, 0.05), 6))));
    out.push_str(&format!("  P10     : {}\n", fmt(round(percentile(&sorted, 0.10), 6))));
    out.push_str(&format!("  P25     : {}\n", fmt(round(percentile(&sorted, 0.25), 6))));
    out.push_str(&format!("  P50     : {}  (median)\n", fmt(round(percentile(&sorted, 0.50), 6))));
    out.push_str(&format!("  P75     : {}\n", fmt(round(percentile(&sorted, 0.75), 6))));
    out.push_str(&format!("  P90     : {}\n", fmt(round(percentile(&sorted, 0.90), 6))));
    out.push_str(&format!("  P95     : {}\n", fmt(round(percentile(&sorted, 0.95), 6))));
    out.push_str(&format!("  P99     : {}\n", fmt(round(percentile(&sorted, 0.99), 6))));

    if let Some(t) = threshold {
        let hits = match direction {
            "at-or-below" => sorted.iter().filter(|&&y| y <= t).count(),
            _ => sorted.iter().filter(|&&y| y >= t).count(),
        };
        let sym = if direction == "at-or-below" { "<=" } else { ">=" };
        let p = hits as f64 / n as f64;
        out.push_str(&format!("\nP(outcome {} {}) = {}%\n", sym, fmt(t), fmt(round(p * 100.0, 4))));
    }

    if bins > 0 {
        let h = histogram(&sorted, bins as usize);
        if !h.is_empty() {
            out.push('\n');
            out.push_str(&h);
            out.push('\n');
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constant_model_is_exact() {
        // Every trial is identical → all stats collapse to the constant.
        let r = simulate("x = constant(5)", "x * 2", 100, 42, None, "").unwrap();
        assert_eq!(r.trials, 100);
        assert_eq!(r.mean, 10.0);
        assert_eq!(r.std_dev, 0.0);
        assert_eq!(r.min, 10.0);
        assert_eq!(r.max, 10.0);
        assert_eq!(r.p5, 10.0);
        assert_eq!(r.p50, 10.0);
        assert_eq!(r.p99, 10.0);
    }

    #[test]
    fn uniform_mean_is_near_analytic() {
        // uniform(0,10) → mean 5, and the outcome stays within [0,10].
        let r = simulate("x = uniform(0, 10)", "x", 20000, 7, None, "").unwrap();
        assert!((r.mean - 5.0).abs() < 0.15, "mean {}", r.mean);
        assert!(r.min >= 0.0 && r.max <= 10.0);
        assert!(r.p50 > 4.0 && r.p50 < 6.0);
    }

    #[test]
    fn normal_std_dev_is_near_analytic() {
        let r = simulate("x = normal(100, 15)", "x", 40000, 123, None, "").unwrap();
        assert!((r.mean - 100.0).abs() < 0.6, "mean {}", r.mean);
        assert!((r.std_dev - 15.0).abs() < 0.6, "std {}", r.std_dev);
    }

    #[test]
    fn reproducible_with_same_seed() {
        let a = simulate("x = normal(0, 1)", "x", 5000, 99, None, "").unwrap();
        let b = simulate("x = normal(0, 1)", "x", 5000, 99, None, "").unwrap();
        assert_eq!(a, b, "same seed must be bit-identical");
        let c = simulate("x = normal(0, 1)", "x", 5000, 100, None, "").unwrap();
        assert_ne!(a.mean, c.mean, "different seed must differ");
    }

    #[test]
    fn threshold_probability_at_or_above() {
        // uniform(0,100): P(>= 75) ~ 0.25.
        let r = simulate("x = uniform(0, 100)", "x", 40000, 5, Some(75.0), "at-or-above").unwrap();
        let p = r.threshold_probability.unwrap();
        assert!((p - 0.25).abs() < 0.02, "p {p}");
        assert_eq!(r.threshold_direction.as_deref(), Some("at-or-above"));
    }

    #[test]
    fn threshold_probability_at_or_below() {
        let r = simulate("x = uniform(0, 100)", "x", 40000, 5, Some(25.0), "at-or-below").unwrap();
        let p = r.threshold_probability.unwrap();
        assert!((p - 0.25).abs() < 0.02, "p {p}");
    }

    #[test]
    fn multi_variable_model() {
        // revenue - cost, both constant → deterministic 400.
        let r = simulate(
            "revenue = constant(1000)\ncost = constant(600)",
            "revenue - cost",
            500,
            1,
            None,
            "",
        )
        .unwrap();
        assert_eq!(r.mean, 400.0);
    }

    #[test]
    fn triangular_stays_in_bounds() {
        let r = simulate("x = triangular(1, 5, 9)", "x", 20000, 3, None, "").unwrap();
        assert!(r.min >= 1.0 && r.max <= 9.0);
        assert!((r.mean - 5.0).abs() < 0.2, "mean {}", r.mean); // symmetric → ~mode
    }

    #[test]
    fn comments_and_blank_lines_ignored() {
        let r = simulate(
            "# a comment\n\nx = constant(3)\n\n# another\n",
            "x + 1",
            100,
            1,
            None,
            "",
        )
        .unwrap();
        assert_eq!(r.mean, 4.0);
    }

    #[test]
    fn report_includes_sections() {
        let out = report("x = uniform(0, 10)", "x", 100, 42, Some(0.0), "at-or-above", 5).unwrap();
        assert!(out.contains("Monte Carlo simulation — 100 trials (seed 42)"));
        assert!(out.contains("Percentiles:"));
        assert!(out.contains("P(outcome >= 0) = 100%"));
        assert!(out.contains("Histogram:"));
    }

    #[test]
    fn err_no_variables() {
        let e = simulate("", "x", 1000, 1, None, "").unwrap_err();
        assert!(e.contains("no input variables"), "{e}");
    }

    #[test]
    fn err_unknown_distribution() {
        let e = simulate("x = weibull(1, 2)", "x", 1000, 1, None, "").unwrap_err();
        assert!(e.contains("unknown distribution"), "{e}");
    }

    #[test]
    fn err_model_unknown_variable() {
        let e = simulate("x = normal(0, 1)", "x + y", 1000, 1, None, "").unwrap_err();
        assert!(e.contains("unknown variable"), "{e}");
    }

    #[test]
    fn err_bad_normal_sd() {
        let e = simulate("x = normal(0, 0)", "x", 1000, 1, None, "").unwrap_err();
        assert!(e.contains("std_dev > 0"), "{e}");
    }

    #[test]
    fn err_trials_out_of_range() {
        assert!(simulate("x = normal(0,1)", "x", 10, 1, None, "")
            .unwrap_err()
            .contains("trials must be between"));
        assert!(simulate("x = normal(0,1)", "x", 2_000_000, 1, None, "")
            .unwrap_err()
            .contains("trials must be between"));
    }

    #[test]
    fn err_bad_direction() {
        let e = simulate("x = normal(0,1)", "x", 1000, 1, Some(1.0), "sideways").unwrap_err();
        assert!(e.contains("threshold_direction"), "{e}");
    }

    #[test]
    fn err_bins_out_of_range() {
        let e = report("x = normal(0,1)", "x", 1000, 1, None, "", 100).unwrap_err();
        assert!(e.contains("histogram_bins"), "{e}");
    }
}
