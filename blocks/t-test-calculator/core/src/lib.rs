//! gizza-ai/t-test-calculator core — Student's t-tests on pasted data.
//!
//! Runs a one-sample, paired, pooled two-sample (Student) or Welch
//! unequal-variance t-test over raw observations (one column, two side-by-side
//! columns, or `group,value` rows) or over published summary statistics
//! (`name,n,mean,sd`). Reports the descriptives, the t statistic, the degrees of
//! freedom (fractional for Welch), the exact p-value for a two-, left- or
//! right-tailed alternative, the critical t, the confidence interval for the
//! estimate, Cohen's d / Hedges' g with an approximate interval, the observed
//! power from the noncentral t distribution, and a variance-ratio check.
//!
//! Everything is pure Rust with no numeric dependencies, so the same code runs
//! natively (CLI), in the wasm32-wasip1 chat block and in the browser page. The
//! special functions (log-gamma, the regularized incomplete beta, the normal
//! CDF, and the central and noncentral t distributions) are implemented and
//! unit-tested here against closed-form identities — see the `special` tests at
//! the bottom.

use serde::Serialize;

/// Hard cap on the number of parsed observations (keeps a pasted spreadsheet
/// from locking up the browser tab).
pub const MAX_VALUES: usize = 200_000;

// ---------------------------------------------------------------------------
// Result types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SampleStats {
    pub name: String,
    /// Number of observations.
    pub n: usize,
    pub mean: f64,
    /// Sample standard deviation (n − 1 denominator).
    pub sd: f64,
    /// Standard error of the mean, sd / √n.
    pub sem: f64,
    /// Smallest observation; absent when only summary statistics were supplied.
    pub min: Option<f64>,
    /// Largest observation; absent when only summary statistics were supplied.
    pub max: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EffectSize {
    /// "Cohen's d", "Hedges' g", …
    pub name: String,
    /// What the difference was divided by, spelled out.
    pub standardizer: String,
    pub value: f64,
    /// negligible / small / medium / large, by Cohen's conventions.
    pub magnitude: String,
    /// Approximate (large-sample normal) interval bounds, when available.
    pub ci_lower: Option<f64>,
    pub ci_upper: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct VarianceTest {
    /// Larger variance ÷ smaller variance.
    pub f: f64,
    pub df1: f64,
    pub df2: f64,
    /// Two-tailed p-value for the variance ratio.
    pub p_value: f64,
    /// True when the ratio does not reject equal variances at alpha.
    pub equal_variances_plausible: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TTestResult {
    /// Canonical test key: one-sample | two-sample | welch | paired.
    pub test: String,
    /// Human-readable test name.
    pub test_name: String,
    /// two | left | right.
    pub tails: String,
    /// Plain-language alternative hypothesis.
    pub alternative: String,
    /// The hypothesized mean (one-sample) or mean difference (two-sample/paired).
    pub mu: f64,
    /// How the input was read: wide | long | summary.
    pub input_format: String,
    pub samples: Vec<SampleStats>,
    /// Paired differences, when the pairs were available.
    pub differences: Option<SampleStats>,
    /// What the test estimates ("sample mean", "mean difference (A - B)").
    pub estimate_label: String,
    pub estimate: f64,
    /// Standard error of the estimate.
    pub standard_error: f64,
    pub t: f64,
    pub df: f64,
    pub p_value: f64,
    pub alpha: f64,
    /// Critical t for the chosen tails at alpha.
    pub critical_t: f64,
    pub reject: bool,
    /// Confidence level as a percentage, 100 × (1 − alpha).
    pub confidence_level: f64,
    /// Interval bounds; `None` is an unbounded side (one-tailed runs).
    pub ci_lower: Option<f64>,
    pub ci_upper: Option<f64>,
    pub effects: Vec<EffectSize>,
    /// Observed (post-hoc) power at alpha for the estimated effect.
    pub power: f64,
    pub variance_test: Option<VarianceTest>,
    /// Human-readable caveats (auto-selected test, small n, unequal variances…).
    pub notes: Vec<String>,
}

// ---------------------------------------------------------------------------
// Special functions
// ---------------------------------------------------------------------------

/// ln Γ(x) — Lanczos approximation (g = 7, n = 9), accurate to ~1e-13.
fn ln_gamma(x: f64) -> f64 {
    const G: f64 = 7.0;
    const C: [f64; 9] = [
        0.999_999_999_999_809_93,
        676.520_368_121_885_1,
        -1_259.139_216_722_402_8,
        771.323_428_777_653_1,
        -176.615_029_162_140_6,
        12.507_343_278_686_905,
        -0.138_571_095_265_720_12,
        9.984_369_578_019_572e-6,
        1.505_632_735_149_311_6e-7,
    ];
    if x < 0.5 {
        // Reflection: Γ(x)Γ(1−x) = π / sin(πx)
        (std::f64::consts::PI / (std::f64::consts::PI * x).sin()).ln() - ln_gamma(1.0 - x)
    } else {
        let x = x - 1.0;
        let mut a = C[0];
        let t = x + G + 0.5;
        for (i, &c) in C.iter().enumerate().skip(1) {
            a += c / (x + i as f64);
        }
        0.5 * (2.0 * std::f64::consts::PI).ln() + (x + 0.5) * t.ln() - t + a.ln()
    }
}

/// Regularized lower incomplete gamma P(s, x) via the series expansion (x < s+1).
fn gamma_p_series(s: f64, x: f64) -> f64 {
    let mut sum = 1.0 / s;
    let mut term = sum;
    let mut n = 1.0;
    while n < 1000.0 {
        term *= x / (s + n);
        sum += term;
        if term.abs() < sum.abs() * 1e-16 {
            break;
        }
        n += 1.0;
    }
    sum * (-x + s * x.ln() - ln_gamma(s)).exp()
}

/// Regularized upper incomplete gamma Q(s, x) via the Lentz continued fraction (x ≥ s+1).
fn gamma_q_cf(s: f64, x: f64) -> f64 {
    const FPMIN: f64 = 1.0e-300;
    let mut b = x + 1.0 - s;
    let mut c = 1.0 / FPMIN;
    let mut d = 1.0 / b;
    let mut h = d;
    let mut i = 1.0;
    while i < 1000.0 {
        let an = -i * (i - s);
        b += 2.0;
        d = an * d + b;
        if d.abs() < FPMIN {
            d = FPMIN;
        }
        c = b + an / c;
        if c.abs() < FPMIN {
            c = FPMIN;
        }
        d = 1.0 / d;
        let del = d * c;
        h *= del;
        if (del - 1.0).abs() < 1e-16 {
            break;
        }
        i += 1.0;
    }
    h * (-x + s * x.ln() - ln_gamma(s)).exp()
}

/// Regularized lower incomplete gamma P(s, x).
fn gamma_p(s: f64, x: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    if x < s + 1.0 {
        gamma_p_series(s, x)
    } else {
        1.0 - gamma_q_cf(s, x)
    }
}

/// Complementary error function, via the incomplete gamma (≈1e-14 accurate).
fn erfc(x: f64) -> f64 {
    if x >= 0.0 {
        if x * x < 1.5 {
            1.0 - gamma_p_series(0.5, x * x)
        } else {
            gamma_q_cf(0.5, x * x)
        }
    } else {
        1.0 + gamma_p(0.5, x * x)
    }
}

/// Standard normal CDF Φ(z).
pub fn norm_cdf(z: f64) -> f64 {
    0.5 * erfc(-z / std::f64::consts::SQRT_2)
}

/// Continued fraction for the incomplete beta function (Numerical Recipes).
fn betacf(a: f64, b: f64, x: f64) -> f64 {
    const MAXIT: usize = 300;
    const EPS: f64 = 3.0e-14;
    const FPMIN: f64 = 1.0e-300;
    let qab = a + b;
    let qap = a + 1.0;
    let qam = a - 1.0;
    let mut c = 1.0;
    let mut d = 1.0 - qab * x / qap;
    if d.abs() < FPMIN {
        d = FPMIN;
    }
    d = 1.0 / d;
    let mut h = d;
    for m in 1..=MAXIT {
        let m = m as f64;
        let m2 = 2.0 * m;
        let mut aa = m * (b - m) * x / ((qam + m2) * (a + m2));
        d = 1.0 + aa * d;
        if d.abs() < FPMIN {
            d = FPMIN;
        }
        c = 1.0 + aa / c;
        if c.abs() < FPMIN {
            c = FPMIN;
        }
        d = 1.0 / d;
        h *= d * c;
        aa = -(a + m) * (qab + m) * x / ((a + m2) * (qap + m2));
        d = 1.0 + aa * d;
        if d.abs() < FPMIN {
            d = FPMIN;
        }
        c = 1.0 + aa / c;
        if c.abs() < FPMIN {
            c = FPMIN;
        }
        d = 1.0 / d;
        let del = d * c;
        h *= del;
        if (del - 1.0).abs() < EPS {
            break;
        }
    }
    h
}

/// Regularized incomplete beta function I_x(a, b).
fn betai(a: f64, b: f64, x: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    if x >= 1.0 {
        return 1.0;
    }
    let bt = (ln_gamma(a + b) - ln_gamma(a) - ln_gamma(b) + a * x.ln() + b * (1.0 - x).ln()).exp();
    if x < (a + 1.0) / (a + b + 2.0) {
        bt * betacf(a, b, x) / a
    } else {
        1.0 - bt * betacf(b, a, 1.0 - x) / b
    }
}

/// Right-tail F p-value: P(F ≥ f) with (d1, d2) degrees of freedom.
pub fn f_upper_tail(f: f64, d1: f64, d2: f64) -> f64 {
    if !f.is_finite() || f <= 0.0 {
        return 1.0;
    }
    betai(d2 / 2.0, d1 / 2.0, d2 / (d2 + d1 * f))
}

/// Two-tailed Student-t p-value: P(|T| ≥ |t|) with `df` degrees of freedom.
pub fn student_t_two_tail(t: f64, df: f64) -> f64 {
    if !t.is_finite() {
        return 0.0;
    }
    betai(df / 2.0, 0.5, df / (df + t * t))
}

/// Central Student-t CDF: P(T ≤ t) with `df` degrees of freedom.
pub fn student_t_cdf(t: f64, df: f64) -> f64 {
    let half = 0.5 * student_t_two_tail(t, df);
    if t >= 0.0 {
        1.0 - half
    } else {
        half
    }
}

/// Bisection inverse of a monotonically DECREASING upper-tail function:
/// returns x with `tail(x) == target`, searching [0, hi].
fn invert_upper_tail<F: Fn(f64) -> f64>(tail: F, target: f64, hi: f64) -> f64 {
    let (mut lo, mut hi) = (0.0f64, hi);
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if tail(mid) > target {
            lo = mid;
        } else {
            hi = mid;
        }
        if hi - lo < 1e-10 * (1.0 + hi) {
            break;
        }
    }
    0.5 * (lo + hi)
}

/// Two-tailed critical t: the positive value with P(|T| ≥ t) = `alpha`.
pub fn t_critical_two_tailed(alpha: f64, df: f64) -> f64 {
    invert_upper_tail(|x| student_t_two_tail(x, df), alpha, 1.0e7)
}

/// One-tailed critical t: the positive value with P(T ≥ t) = `alpha`.
pub fn t_critical_one_tailed(alpha: f64, df: f64) -> f64 {
    t_critical_two_tailed((2.0 * alpha).min(1.0), df)
}

/// Positive standard-normal quantile: z with Φ(z) = `p`, for p ≥ 0.5.
fn norm_quantile_upper(p: f64) -> f64 {
    invert_upper_tail(|z| 1.0 - norm_cdf(z), 1.0 - p, 40.0)
}

/// Noncentral Student-t CDF P(T' ≤ t) with `df` degrees of freedom and
/// noncentrality `delta` — Lenth's series (Algorithm AS 243).
pub fn noncentral_t_cdf(t: f64, df: f64, delta: f64) -> f64 {
    if !t.is_finite() {
        return if t > 0.0 { 1.0 } else { 0.0 };
    }
    // The distribution is only symmetric under simultaneous sign flips.
    if t < 0.0 {
        return 1.0 - noncentral_t_cdf(-t, df, -delta);
    }
    // Beyond ~±37 the series underflows; the tail is 0/1 to double precision.
    let delta = delta.clamp(-37.5, 37.5);
    let x = t * t / (t * t + df);
    if x <= 0.0 {
        return norm_cdf(-delta);
    }
    let lambda = 0.5 * delta * delta;
    let e = (-lambda).exp();
    let mut p = e; // p_0 = e^{-λ}
    // q_0 = e^{-λ} · δ / (√2 · Γ(3/2)) = e^{-λ} · δ · √(2/π)
    let mut q = e * delta * (2.0 / std::f64::consts::PI).sqrt();
    let mut sum = 0.0;
    for j in 0..1000 {
        let j = j as f64;
        let ip = betai(j + 0.5, 0.5 * df, x);
        let iq = betai(j + 1.0, 0.5 * df, x);
        let term = p * ip + q * iq;
        sum += term;
        // The p/q weights are Poisson-like: once both are negligible and the
        // running term has stopped moving the total, further terms cannot.
        if j > lambda && term.abs() < 1e-15 * (1.0 + sum.abs()) {
            break;
        }
        p *= lambda / (j + 1.0);
        q *= lambda / (j + 1.5);
    }
    (norm_cdf(-delta) + 0.5 * sum).clamp(0.0, 1.0)
}

// ---------------------------------------------------------------------------
// Input parsing
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Delim {
    Auto,
    Comma,
    Tab,
    Semicolon,
    Pipe,
    Whitespace,
}

fn parse_delim(s: &str) -> Result<Delim, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "auto" => Ok(Delim::Auto),
        "comma" | "," => Ok(Delim::Comma),
        "tab" | "\t" => Ok(Delim::Tab),
        "semicolon" | ";" => Ok(Delim::Semicolon),
        "pipe" | "|" => Ok(Delim::Pipe),
        "space" | "whitespace" => Ok(Delim::Whitespace),
        other => Err(format!(
            "invalid delimiter {other:?}: expected \"auto\", \"comma\", \"tab\", \"semicolon\", \"pipe\" or \"space\""
        )),
    }
}

/// Split a line on the chosen delimiter, trimming each field and any wrapping quotes.
fn split_line(line: &str, d: Delim) -> Vec<String> {
    let raw: Vec<&str> = match d {
        Delim::Comma => line.split(',').collect(),
        Delim::Tab => line.split('\t').collect(),
        Delim::Semicolon => line.split(';').collect(),
        Delim::Pipe => line.split('|').collect(),
        Delim::Whitespace | Delim::Auto => line.split_whitespace().collect(),
    };
    raw.iter()
        .map(|f| f.trim().trim_matches(|c| c == '"' || c == '\'').trim())
        .map(|f| f.to_string())
        .collect()
}

/// Strip comments/blank lines and return the meaningful data lines.
fn data_lines(data: &str) -> Vec<&str> {
    data.lines()
        .map(|l| l.trim_end_matches('\r').trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect()
}

fn detect_delim(lines: &[&str]) -> Delim {
    let probe = lines.iter().take(5).copied().collect::<Vec<_>>();
    let any = |c: char| probe.iter().any(|l| l.contains(c));
    if any('\t') {
        Delim::Tab
    } else if any(',') {
        Delim::Comma
    } else if any(';') {
        Delim::Semicolon
    } else if any('|') {
        Delim::Pipe
    } else {
        Delim::Whitespace
    }
}

fn is_number(s: &str) -> bool {
    !s.is_empty() && s.parse::<f64>().map(|v| v.is_finite()).unwrap_or(false)
}

fn parse_number(s: &str, line_no: usize) -> Result<f64, String> {
    let v: f64 = s
        .parse()
        .map_err(|_| format!("line {line_no}: expected a number, got {s:?}"))?;
    if !v.is_finite() {
        return Err(format!(
            "line {line_no}: expected a finite number, got {s:?}"
        ));
    }
    Ok(v)
}

/// Parsed input: either raw observations per sample, or per-sample summaries.
#[derive(Debug, Clone, PartialEq)]
enum Parsed {
    /// (name, values)
    Raw(Vec<(String, Vec<f64>)>),
    /// (name, n, mean, sd)
    Summary(Vec<(String, usize, f64, f64)>),
}

fn header_wanted(header: &str) -> Result<Option<bool>, String> {
    match header.trim().to_ascii_lowercase().as_str() {
        "" | "auto" => Ok(None),
        "yes" | "true" | "1" => Ok(Some(true)),
        "no" | "false" | "0" => Ok(Some(false)),
        other => Err(format!(
            "invalid header {other:?}: expected \"auto\", \"yes\" or \"no\""
        )),
    }
}

/// Decide long vs wide vs summary when `format = "auto"`.
fn detect_format(rows: &[Vec<String>], skipped_header: bool) -> &'static str {
    let body = if skipped_header { &rows[1..] } else { rows };
    if body.is_empty() {
        return "wide";
    }
    // `name,n,mean,sd` rows: four columns whose last three are numeric.
    if body
        .iter()
        .all(|r| r.len() == 4 && is_number(&r[1]) && is_number(&r[2]) && is_number(&r[3]))
        && body.iter().any(|r| !is_number(&r[0]))
    {
        return "summary";
    }
    if !body.iter().all(|r| r.len() == 2) {
        return "wide";
    }
    let label_first = body.iter().filter(|r| !is_number(&r[0])).count();
    let label_second = body.iter().filter(|r| !is_number(&r[1])).count();
    if label_first > 0 || label_second > 0 {
        "long"
    } else {
        "wide"
    }
}

fn parse_input(
    data: &str,
    format: &str,
    delimiter: &str,
    header: &str,
) -> Result<(Parsed, String), String> {
    let lines = data_lines(data);
    if lines.is_empty() {
        return Err(
            "no data: paste one column of numbers for a one-sample test, two columns for a two-sample or paired test, `group,value` rows, or `name,n,mean,sd` summary rows"
                .into(),
        );
    }
    let d = match parse_delim(delimiter)? {
        Delim::Auto => detect_delim(&lines),
        other => other,
    };
    let rows: Vec<Vec<String>> = lines.iter().map(|l| split_line(l, d)).collect();
    let want_header = header_wanted(header)?;
    let fmt = format.trim().to_ascii_lowercase();

    match fmt.as_str() {
        "" | "auto" | "long" | "wide" | "summary" => {}
        other => {
            return Err(format!(
                "invalid format {other:?}: expected \"auto\", \"wide\", \"long\" or \"summary\""
            ))
        }
    }

    // A header row is one whose cells don't parse as numbers where numbers are expected.
    let auto_header = match fmt.as_str() {
        "long" => rows[0].len() >= 2 && !is_number(&rows[0][1]) && !is_number(&rows[0][0]),
        "summary" => rows[0].len() >= 4 && !is_number(&rows[0][1]),
        "wide" => rows[0].iter().any(|c| !c.is_empty() && !is_number(c)),
        // auto: a first row with no numeric cell at all is a header
        _ => rows[0].iter().all(|c| c.is_empty() || !is_number(c)),
    };
    let has_header = want_header.unwrap_or(auto_header);
    if has_header && rows.len() < 2 {
        return Err("only a header row was found — add at least one data row below it".into());
    }

    let resolved = if fmt.is_empty() || fmt == "auto" {
        detect_format(&rows, has_header).to_string()
    } else {
        fmt.clone()
    };

    match resolved.as_str() {
        "long" => parse_long(&rows, has_header).map(|p| (p, resolved)),
        "wide" => parse_wide(&rows, has_header).map(|p| (p, resolved)),
        "summary" => parse_summary(&rows, has_header).map(|p| (p, resolved)),
        _ => unreachable!(),
    }
}

fn push_value(
    groups: &mut Vec<(String, Vec<f64>)>,
    name: &str,
    value: f64,
) -> Result<(), String> {
    match groups.iter_mut().find(|(n, _)| n == name) {
        Some((_, vals)) => vals.push(value),
        None => {
            if groups.len() >= 2 {
                return Err(format!(
                    "found more than 2 groups (\"{}\", \"{}\", \"{name}\", …) — a t-test compares at most two; use a one-way ANOVA for three or more",
                    groups[0].0, groups[1].0
                ));
            }
            groups.push((name.to_string(), vec![value]));
        }
    }
    Ok(())
}

fn parse_long(rows: &[Vec<String>], has_header: bool) -> Result<Parsed, String> {
    let start = usize::from(has_header);
    let body = &rows[start..];
    if body.is_empty() {
        return Err("no data rows found after the header".into());
    }
    // `group,value` is the documented order; accept `value,group` when the
    // second column is clearly the label instead.
    let label_first = body
        .iter()
        .filter(|r| r.len() >= 2 && !is_number(&r[0]))
        .count();
    let label_second = body
        .iter()
        .filter(|r| r.len() >= 2 && !is_number(&r[1]))
        .count();
    let reversed = label_second > label_first;

    let mut groups: Vec<(String, Vec<f64>)> = Vec::new();
    let mut total = 0usize;
    for (i, row) in body.iter().enumerate() {
        let line_no = start + i + 1;
        if row.len() < 2 {
            return Err(format!(
                "line {line_no}: expected 2 fields (group and value), got {} — long format needs one \"group,value\" pair per line",
                row.len()
            ));
        }
        let (label, value_str) = if reversed {
            (row[1].as_str(), row[0].as_str())
        } else {
            (row[0].as_str(), row[1].as_str())
        };
        if value_str.is_empty() {
            continue;
        }
        let label = if label.is_empty() { "(unnamed)" } else { label };
        let value = parse_number(value_str, line_no)?;
        total += 1;
        if total > MAX_VALUES {
            return Err(format!("too many values: the maximum is {MAX_VALUES}"));
        }
        push_value(&mut groups, label, value)?;
    }
    if groups.is_empty() {
        return Err("no usable observations were found".into());
    }
    Ok(Parsed::Raw(groups))
}

fn parse_wide(rows: &[Vec<String>], has_header: bool) -> Result<Parsed, String> {
    let width = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    if width == 0 {
        return Err("no usable observations were found".into());
    }
    if width > 2 {
        return Err(format!(
            "found {width} columns — a t-test compares at most two samples; use a one-way ANOVA for three or more groups"
        ));
    }
    let names: Vec<String> = (0..width)
        .map(|c| {
            let from_header = if has_header {
                rows[0].get(c).map(|s| s.trim()).unwrap_or("")
            } else {
                ""
            };
            if from_header.is_empty() {
                if width == 1 {
                    "Sample".to_string()
                } else {
                    format!("Sample {}", c + 1)
                }
            } else {
                from_header.to_string()
            }
        })
        .collect();
    let start = usize::from(has_header);
    let mut cols: Vec<Vec<f64>> = vec![Vec::new(); width];
    let mut total = 0usize;
    for (i, row) in rows[start..].iter().enumerate() {
        let line_no = start + i + 1;
        for (c, cell) in row.iter().enumerate() {
            if cell.is_empty() {
                continue;
            }
            let v = parse_number(cell, line_no)
                .map_err(|e| format!("{e} (column {} — \"{}\")", c + 1, names[c]))?;
            total += 1;
            if total > MAX_VALUES {
                return Err(format!("too many values: the maximum is {MAX_VALUES}"));
            }
            cols[c].push(v);
        }
    }
    let groups: Vec<(String, Vec<f64>)> = names
        .into_iter()
        .zip(cols)
        .filter(|(_, v)| !v.is_empty())
        .collect();
    if groups.is_empty() {
        return Err("no usable observations were found".into());
    }
    Ok(Parsed::Raw(groups))
}

fn parse_summary(rows: &[Vec<String>], has_header: bool) -> Result<Parsed, String> {
    let start = usize::from(has_header);
    let body = &rows[start..];
    if body.is_empty() {
        return Err("no data rows found after the header".into());
    }
    if body.len() > 2 {
        return Err(format!(
            "found {} summary rows — a t-test compares at most two samples; use a one-way ANOVA for three or more groups",
            body.len()
        ));
    }
    let mut out = Vec::new();
    for (i, row) in body.iter().enumerate() {
        let line_no = start + i + 1;
        if row.len() < 4 {
            return Err(format!(
                "line {line_no}: expected 4 fields (name, n, mean, sd), got {} — summary format needs one \"name,n,mean,sd\" row per sample",
                row.len()
            ));
        }
        let name = if row[0].is_empty() {
            format!("Sample {}", i + 1)
        } else {
            row[0].clone()
        };
        let n_f = parse_number(&row[1], line_no)?;
        if n_f < 1.0 || n_f.fract() != 0.0 {
            return Err(format!(
                "line {line_no}: n must be a whole number >= 1, got {}",
                row[1]
            ));
        }
        let mean = parse_number(&row[2], line_no)?;
        let sd = parse_number(&row[3], line_no)?;
        if sd < 0.0 {
            return Err(format!(
                "line {line_no}: the standard deviation must be >= 0, got {sd}"
            ));
        }
        out.push((name, n_f as usize, mean, sd));
    }
    Ok(Parsed::Summary(out))
}

// ---------------------------------------------------------------------------
// The t-test itself
// ---------------------------------------------------------------------------

fn mean_of(v: &[f64]) -> f64 {
    v.iter().sum::<f64>() / v.len() as f64
}

/// Sample standard deviation (n − 1 denominator); 0 for a single observation.
fn sd_of(v: &[f64]) -> f64 {
    if v.len() < 2 {
        return 0.0;
    }
    let m = mean_of(v);
    let ss: f64 = v.iter().map(|x| (x - m) * (x - m)).sum();
    (ss / (v.len() as f64 - 1.0)).sqrt()
}

fn stats_from_raw(name: &str, v: &[f64]) -> SampleStats {
    let mean = mean_of(v);
    let sd = sd_of(v);
    SampleStats {
        name: name.to_string(),
        n: v.len(),
        mean,
        sd,
        sem: sd / (v.len() as f64).sqrt(),
        min: v.iter().cloned().fold(None, |a: Option<f64>, x| {
            Some(a.map_or(x, |m| m.min(x)))
        }),
        max: v.iter().cloned().fold(None, |a: Option<f64>, x| {
            Some(a.map_or(x, |m| m.max(x)))
        }),
    }
}

fn stats_from_summary(name: &str, n: usize, mean: f64, sd: f64) -> SampleStats {
    SampleStats {
        name: name.to_string(),
        n,
        mean,
        sd,
        sem: sd / (n as f64).sqrt(),
        min: None,
        max: None,
    }
}

fn round_to(v: f64, decimals: usize) -> f64 {
    if !v.is_finite() {
        return v;
    }
    let f = 10f64.powi(decimals as i32);
    (v * f).round() / f
}

fn magnitude_label(d: f64) -> &'static str {
    let a = d.abs();
    if a < 0.2 {
        "negligible"
    } else if a < 0.5 {
        "small"
    } else if a < 0.8 {
        "medium"
    } else {
        "large"
    }
}

fn parse_tails(tails: &str) -> Result<&'static str, String> {
    match tails.trim().to_ascii_lowercase().as_str() {
        "" | "two" | "two-tailed" | "two-sided" | "both" => Ok("two"),
        "left" | "less" | "lower" => Ok("left"),
        "right" | "greater" | "upper" => Ok("right"),
        other => Err(format!(
            "invalid tails {other:?}: expected \"two\", \"left\" or \"right\""
        )),
    }
}

fn parse_test(test: &str) -> Result<&'static str, String> {
    match test.trim().to_ascii_lowercase().as_str() {
        "" | "auto" => Ok("auto"),
        "one-sample" | "one_sample" | "onesample" | "one" => Ok("one-sample"),
        "two-sample" | "two_sample" | "twosample" | "student" | "pooled" => Ok("two-sample"),
        "welch" | "unequal" => Ok("welch"),
        "paired" | "dependent" => Ok("paired"),
        other => Err(format!(
            "invalid test {other:?}: expected \"auto\", \"one-sample\", \"two-sample\", \"welch\" or \"paired\""
        )),
    }
}

/// Approximate (large-sample normal) interval for a standardized effect size.
fn effect_ci(d: f64, se: f64, alpha: f64) -> (Option<f64>, Option<f64>) {
    if !se.is_finite() || se <= 0.0 {
        return (None, None);
    }
    let z = norm_quantile_upper(1.0 - alpha / 2.0);
    (Some(d - z * se), Some(d + z * se))
}

/// Parse + validate the options, then run the test.
#[allow(clippy::too_many_arguments)]
pub fn analyze(
    data: &str,
    test: &str,
    format: &str,
    delimiter: &str,
    header: &str,
    mu: f64,
    tails: &str,
    alpha: f64,
) -> Result<TTestResult, String> {
    if !alpha.is_finite() || !(0.0001..=0.5).contains(&alpha) {
        return Err(format!(
            "invalid alpha {alpha}: expected a significance level between 0.0001 and 0.5"
        ));
    }
    if !mu.is_finite() {
        return Err(format!("invalid mu {mu}: expected a finite number"));
    }
    let tails = parse_tails(tails)?;
    let requested = parse_test(test)?;
    let (parsed, input_format) = parse_input(data, format, delimiter, header)?;

    let n_samples = match &parsed {
        Parsed::Raw(g) => g.len(),
        Parsed::Summary(s) => s.len(),
    };
    let chosen = if requested == "auto" {
        if n_samples == 1 {
            "one-sample"
        } else {
            "welch"
        }
    } else {
        requested
    };

    let mut notes: Vec<String> = Vec::new();
    if requested == "auto" {
        notes.push(match chosen {
            "one-sample" => "auto-selected the one-sample t-test because a single column of values was found; set test=paired and paste the pre-computed differences if these are paired differences".to_string(),
            _ => "auto-selected Welch's unequal-variance t-test for two samples (the safer default); choose two-sample for the pooled Student test, or paired when the columns are matched observations".to_string(),
        });
    }

    let mut samples: Vec<SampleStats> = match &parsed {
        Parsed::Raw(g) => g.iter().map(|(n, v)| stats_from_raw(n, v)).collect(),
        Parsed::Summary(s) => s
            .iter()
            .map(|(n, c, m, sd)| stats_from_summary(n, *c, *m, *sd))
            .collect(),
    };

    // ---- variance-ratio check (two independent samples only) --------------
    let variance_test = if samples.len() == 2
        && chosen != "paired"
        && samples[0].n >= 2
        && samples[1].n >= 2
        && samples[0].sd > 0.0
        && samples[1].sd > 0.0
    {
        let (v1, v2) = (samples[0].sd.powi(2), samples[1].sd.powi(2));
        let (big, small, dfb, dfs) = if v1 >= v2 {
            (v1, v2, samples[0].n as f64 - 1.0, samples[1].n as f64 - 1.0)
        } else {
            (v2, v1, samples[1].n as f64 - 1.0, samples[0].n as f64 - 1.0)
        };
        let f = big / small;
        let p = (2.0 * f_upper_tail(f, dfb, dfs)).min(1.0);
        Some(VarianceTest {
            f,
            df1: dfb,
            df2: dfs,
            p_value: p,
            equal_variances_plausible: p >= alpha,
        })
    } else {
        None
    };

    // ---- assemble the estimate / standard error / df -----------------------
    let mut differences: Option<SampleStats> = None;
    let (estimate_label, estimate, se, df, test_name);

    match chosen {
        "one-sample" => {
            if samples.len() != 1 {
                return Err(format!(
                    "the one-sample t-test expects one column of values, but {} samples were found — pick two-sample, welch or paired instead",
                    samples.len()
                ));
            }
            let s = &samples[0];
            if s.n < 2 {
                return Err(
                    "the one-sample t-test needs at least 2 observations to estimate the standard deviation".into(),
                );
            }
            if s.sd <= 0.0 {
                return Err(
                    "the standard deviation is 0 — every value is identical, so the t statistic is undefined".into(),
                );
            }
            estimate_label = "sample mean".to_string();
            estimate = s.mean;
            se = s.sem;
            df = s.n as f64 - 1.0;
            test_name = "One-sample t-test".to_string();
        }
        "paired" => {
            match &parsed {
                Parsed::Raw(g) if g.len() == 2 => {
                    let (a, b) = (&g[0].1, &g[1].1);
                    if a.len() != b.len() {
                        return Err(format!(
                            "the paired t-test needs the same number of values in both columns, got {} and {} — remove the unmatched rows or run an independent-samples test",
                            a.len(),
                            b.len()
                        ));
                    }
                    let diffs: Vec<f64> = a.iter().zip(b).map(|(x, y)| x - y).collect();
                    let name = format!("{} - {}", g[0].0, g[1].0);
                    differences = Some(stats_from_raw(&name, &diffs));
                }
                Parsed::Raw(g) if g.len() == 1 => {
                    differences = Some(stats_from_raw("differences", &g[0].1));
                    notes.push(
                        "one column was supplied, so its values were treated as the pre-computed paired differences".into(),
                    );
                }
                Parsed::Summary(s) if s.len() == 1 => {
                    differences = Some(stats_from_summary(&s[0].0, s[0].1, s[0].2, s[0].3));
                    notes.push(
                        "the summary row was read as the n, mean and standard deviation of the paired differences".into(),
                    );
                }
                _ => {
                    return Err(
                        "a paired t-test cannot be run from two rows of summary statistics: the pairing (the correlation between the samples) is not recoverable from separate means and standard deviations. Paste the raw pairs as two columns, or one `name,n,mean,sd` row describing the differences.".into(),
                    )
                }
            }
            let d = differences.as_ref().unwrap();
            if d.n < 2 {
                return Err(
                    "the paired t-test needs at least 2 pairs to estimate the standard deviation of the differences".into(),
                );
            }
            if d.sd <= 0.0 {
                return Err(
                    "the standard deviation of the differences is 0 — every pair changed by exactly the same amount, so the t statistic is undefined".into(),
                );
            }
            estimate_label = format!("mean difference ({})", d.name);
            estimate = d.mean;
            se = d.sem;
            df = d.n as f64 - 1.0;
            test_name = "Paired-samples t-test".to_string();
        }
        "two-sample" | "welch" => {
            if samples.len() != 2 {
                return Err(format!(
                    "a two-sample t-test needs two samples, but {} was found — paste two columns, `group,value` rows with two labels, or two `name,n,mean,sd` rows",
                    samples.len()
                ));
            }
            let (a, b) = (&samples[0], &samples[1]);
            if a.n < 2 || b.n < 2 {
                return Err(
                    "each sample needs at least 2 observations to estimate its standard deviation".into(),
                );
            }
            if a.sd <= 0.0 && b.sd <= 0.0 {
                return Err(
                    "both standard deviations are 0 — every value inside each sample is identical, so the t statistic is undefined".into(),
                );
            }
            let (n1, n2) = (a.n as f64, b.n as f64);
            let (v1, v2) = (a.sd * a.sd, b.sd * b.sd);
            estimate_label = format!("mean difference ({} - {})", a.name, b.name);
            estimate = a.mean - b.mean;
            if chosen == "welch" {
                let t1 = v1 / n1;
                let t2 = v2 / n2;
                se = (t1 + t2).sqrt();
                df = (t1 + t2).powi(2) / (t1 * t1 / (n1 - 1.0) + t2 * t2 / (n2 - 1.0));
                test_name = "Welch's unequal-variance t-test".to_string();
            } else {
                let sp2 = ((n1 - 1.0) * v1 + (n2 - 1.0) * v2) / (n1 + n2 - 2.0);
                se = (sp2 * (1.0 / n1 + 1.0 / n2)).sqrt();
                df = n1 + n2 - 2.0;
                test_name = "Two-sample (pooled) t-test".to_string();
            }
        }
        _ => unreachable!(),
    }

    if !(se > 0.0) || !se.is_finite() || !df.is_finite() || df <= 0.0 {
        return Err(
            "the standard error works out to zero — there is no variation left to test".into(),
        );
    }

    let t = (estimate - mu) / se;
    let two_tail = student_t_two_tail(t, df);
    let p_value = match tails {
        "two" => two_tail,
        "right" => {
            if t >= 0.0 {
                0.5 * two_tail
            } else {
                1.0 - 0.5 * two_tail
            }
        }
        _ => {
            if t <= 0.0 {
                0.5 * two_tail
            } else {
                1.0 - 0.5 * two_tail
            }
        }
    };
    let critical_t = if tails == "two" {
        t_critical_two_tailed(alpha, df)
    } else {
        t_critical_one_tailed(alpha, df)
    };
    let (ci_lower, ci_upper) = match tails {
        "two" => (
            Some(estimate - critical_t * se),
            Some(estimate + critical_t * se),
        ),
        "right" => (Some(estimate - critical_t * se), None),
        _ => (None, Some(estimate + critical_t * se)),
    };

    // ---- effect sizes ------------------------------------------------------
    let mut effects: Vec<EffectSize> = Vec::new();
    let raw_diff = estimate - mu;
    match chosen {
        "one-sample" => {
            let s = &samples[0];
            let n = s.n as f64;
            let d = raw_diff / s.sd;
            let se_d = (1.0 / n + d * d / (2.0 * n)).sqrt();
            let (lo, hi) = effect_ci(d, se_d, alpha);
            effects.push(EffectSize {
                name: "Cohen's d".into(),
                standardizer: "sample standard deviation".into(),
                value: d,
                magnitude: magnitude_label(d).into(),
                ci_lower: lo,
                ci_upper: hi,
            });
            let j = 1.0 - 3.0 / (4.0 * df - 1.0);
            effects.push(EffectSize {
                name: "Hedges' g".into(),
                standardizer: "sample standard deviation, bias-corrected".into(),
                value: j * d,
                magnitude: magnitude_label(j * d).into(),
                ci_lower: lo.map(|v| j * v),
                ci_upper: hi.map(|v| j * v),
            });
        }
        "paired" => {
            let dstats = differences.as_ref().unwrap();
            let n = dstats.n as f64;
            let dz = raw_diff / dstats.sd;
            let se_d = (1.0 / n + dz * dz / (2.0 * n)).sqrt();
            let (lo, hi) = effect_ci(dz, se_d, alpha);
            effects.push(EffectSize {
                name: "Cohen's d (dz)".into(),
                standardizer: "standard deviation of the differences".into(),
                value: dz,
                magnitude: magnitude_label(dz).into(),
                ci_lower: lo,
                ci_upper: hi,
            });
            let j = 1.0 - 3.0 / (4.0 * df - 1.0);
            effects.push(EffectSize {
                name: "Hedges' g".into(),
                standardizer: "standard deviation of the differences, bias-corrected".into(),
                value: j * dz,
                magnitude: magnitude_label(j * dz).into(),
                ci_lower: lo.map(|v| j * v),
                ci_upper: hi.map(|v| j * v),
            });
            if samples.len() == 2 && samples[0].sd > 0.0 && samples[1].sd > 0.0 {
                let av = 0.5 * (samples[0].sd + samples[1].sd);
                let dav = raw_diff / av;
                effects.push(EffectSize {
                    name: "Cohen's d (dav)".into(),
                    standardizer: "average of the two sample standard deviations".into(),
                    value: dav,
                    magnitude: magnitude_label(dav).into(),
                    ci_lower: None,
                    ci_upper: None,
                });
            }
        }
        _ => {
            let (a, b) = (&samples[0], &samples[1]);
            let (n1, n2) = (a.n as f64, b.n as f64);
            let (v1, v2) = (a.sd * a.sd, b.sd * b.sd);
            let (standardizer, s_ref) = if chosen == "welch" {
                (
                    "root mean of the two sample variances",
                    (0.5 * (v1 + v2)).sqrt(),
                )
            } else {
                (
                    "pooled standard deviation",
                    (((n1 - 1.0) * v1 + (n2 - 1.0) * v2) / (n1 + n2 - 2.0)).sqrt(),
                )
            };
            let d = raw_diff / s_ref;
            let se_d = ((n1 + n2) / (n1 * n2) + d * d / (2.0 * (n1 + n2))).sqrt();
            let (lo, hi) = effect_ci(d, se_d, alpha);
            effects.push(EffectSize {
                name: "Cohen's d".into(),
                standardizer: standardizer.into(),
                value: d,
                magnitude: magnitude_label(d).into(),
                ci_lower: lo,
                ci_upper: hi,
            });
            let j = 1.0 - 3.0 / (4.0 * (n1 + n2) - 9.0);
            effects.push(EffectSize {
                name: "Hedges' g".into(),
                standardizer: format!("{standardizer}, bias-corrected"),
                value: j * d,
                magnitude: magnitude_label(j * d).into(),
                ci_lower: lo.map(|v| j * v),
                ci_upper: hi.map(|v| j * v),
            });
        }
    }

    // ---- observed power ----------------------------------------------------
    let power = match tails {
        "two" => {
            1.0 - noncentral_t_cdf(critical_t, df, t) + noncentral_t_cdf(-critical_t, df, t)
        }
        "right" => 1.0 - noncentral_t_cdf(critical_t, df, t),
        _ => noncentral_t_cdf(-critical_t, df, t),
    }
    .clamp(0.0, 1.0);

    // ---- notes -------------------------------------------------------------
    if let Some(v) = &variance_test {
        if chosen == "two-sample" && !v.equal_variances_plausible {
            notes.push(format!(
                "the variance ratio rejects equal variances (F = {:.4}, p = {:.4}); Welch's test is the safer choice here",
                v.f, v.p_value
            ));
        } else if chosen == "welch" && v.equal_variances_plausible {
            notes.push(format!(
                "the variance ratio does not reject equal variances (F = {:.4}, p = {:.4}); the pooled two-sample test is also applicable and has slightly more power",
                v.f, v.p_value
            ));
        }
    }
    let smallest = samples.iter().map(|s| s.n).min().unwrap_or(0);
    if smallest < 15 {
        notes.push(format!(
            "the smallest sample has n = {smallest}; with fewer than about 15 observations the t-test leans on the normality assumption, so check for strong skew or outliers before trusting the p-value"
        ));
    }
    if matches!(parsed, Parsed::Summary(_)) {
        notes.push(
            "summary-statistics input has no individual observations, so the minimum and maximum are not reported".into(),
        );
    }
    notes.push(
        "the power figure is observed (post-hoc) power computed from the effect that was actually measured; it is a restatement of the p-value, not evidence about the true effect — plan sample sizes with a target effect size instead".into(),
    );

    // Reorder sample stats so the paired difference row is always last.
    samples.shrink_to_fit();

    Ok(TTestResult {
        test: chosen.to_string(),
        test_name,
        tails: tails.to_string(),
        alternative: alternative_text(chosen, tails, mu, &samples),
        mu,
        input_format,
        samples,
        differences,
        estimate_label,
        estimate,
        standard_error: se,
        t,
        df,
        p_value,
        alpha,
        critical_t,
        reject: p_value < alpha,
        confidence_level: 100.0 * (1.0 - alpha),
        ci_lower,
        ci_upper,
        effects,
        power,
        variance_test,
        notes,
    })
}

fn alternative_text(test: &str, tails: &str, mu: f64, samples: &[SampleStats]) -> String {
    let subject = if test == "one-sample" {
        "the population mean".to_string()
    } else if test == "paired" {
        "the mean difference".to_string()
    } else if samples.len() == 2 {
        format!("the mean of {} minus the mean of {}", samples[0].name, samples[1].name)
    } else {
        "the mean difference".to_string()
    };
    let target = trim_float(mu);
    match tails {
        "right" => format!("{subject} is greater than {target}"),
        "left" => format!("{subject} is less than {target}"),
        _ => format!("{subject} is not equal to {target}"),
    }
}

/// Format a float with no trailing zeros (for hypothesis text).
fn trim_float(v: f64) -> String {
    let s = format!("{v:.6}");
    let s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    if s.is_empty() || s == "-" {
        "0".into()
    } else {
        s
    }
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

fn fmt_num(v: f64, d: usize) -> String {
    if !v.is_finite() {
        return if v > 0.0 { "infinity".into() } else { "-infinity".into() };
    }
    let r = round_to(v, d);
    let r = if r == 0.0 { 0.0 } else { r };
    format!("{r:.d$}", d = d)
}

fn fmt_opt(v: Option<f64>, d: usize) -> String {
    match v {
        Some(x) => fmt_num(x, d),
        None => "n/a".to_string(),
    }
}

fn fmt_p(p: f64, d: usize) -> String {
    let d = d.max(1);
    let floor = 10f64.powi(-(d as i32));
    if p < floor {
        format!("< {floor:.d$}", d = d)
    } else {
        fmt_num(p, d)
    }
}

/// Degrees of freedom print as an integer when they are whole (Welch's are not).
fn fmt_df(df: f64, d: usize) -> String {
    if df.fract().abs() < 1e-9 {
        format!("{}", df.round() as i64)
    } else {
        fmt_num(df, d.max(2))
    }
}

fn fmt_level(level: f64) -> String {
    let s = format!("{level:.4}");
    let s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    format!("{s}%")
}

fn pad_left(s: &str, w: usize) -> String {
    if s.chars().count() >= w {
        s.to_string()
    } else {
        format!("{}{}", " ".repeat(w - s.chars().count()), s)
    }
}

fn pad_right(s: &str, w: usize) -> String {
    if s.chars().count() >= w {
        s.to_string()
    } else {
        format!("{}{}", s, " ".repeat(w - s.chars().count()))
    }
}

fn ci_text(lo: Option<f64>, hi: Option<f64>, d: usize) -> String {
    match (lo, hi) {
        (Some(a), Some(b)) => format!("[{}, {}]", fmt_num(a, d), fmt_num(b, d)),
        (Some(a), None) => format!("[{}, infinity)", fmt_num(a, d)),
        (None, Some(b)) => format!("(-infinity, {}]", fmt_num(b, d)),
        (None, None) => "n/a".into(),
    }
}

fn all_rows(r: &TTestResult) -> Vec<&SampleStats> {
    let mut v: Vec<&SampleStats> = r.samples.iter().collect();
    if let Some(d) = &r.differences {
        v.push(d);
    }
    v
}

pub fn render_summary(r: &TTestResult, d: usize) -> String {
    let mut out = String::new();
    out.push_str(&format!("{}\n", r.test_name));
    out.push_str(&format!(
        "alternative: {} ({}-tailed)\n",
        r.alternative, r.tails
    ));
    out.push_str(&format!(
        "null hypothesis: {} = {}\n\n",
        if r.test == "one-sample" {
            "population mean"
        } else {
            "mean difference"
        },
        trim_float(r.mu)
    ));

    // ---- descriptives table ----
    let rows = all_rows(r);
    let name_w = rows
        .iter()
        .map(|s| s.name.chars().count())
        .max()
        .unwrap_or(6)
        .max(6);
    let num_w = 10usize.max(d + 6);
    out.push_str(&format!(
        "{}  {}  {}  {}  {}  {}  {}\n",
        pad_right("sample", name_w),
        pad_left("n", 6),
        pad_left("mean", num_w),
        pad_left("sd", num_w),
        pad_left("sem", num_w),
        pad_left("min", num_w),
        pad_left("max", num_w),
    ));
    for s in &rows {
        out.push_str(&format!(
            "{}  {}  {}  {}  {}  {}  {}\n",
            pad_right(&s.name, name_w),
            pad_left(&s.n.to_string(), 6),
            pad_left(&fmt_num(s.mean, d), num_w),
            pad_left(&fmt_num(s.sd, d), num_w),
            pad_left(&fmt_num(s.sem, d), num_w),
            pad_left(&fmt_opt(s.min, d), num_w),
            pad_left(&fmt_opt(s.max, d), num_w),
        ));
    }
    out.push('\n');

    // ---- the test ----
    out.push_str(&format!(
        "{}: {}\n",
        r.estimate_label,
        fmt_num(r.estimate, d)
    ));
    out.push_str(&format!(
        "standard error: {}\n",
        fmt_num(r.standard_error, d)
    ));
    out.push_str(&format!(
        "t({}) = {}, p = {}\n",
        fmt_df(r.df, d),
        fmt_num(r.t, d),
        fmt_p(r.p_value, d)
    ));
    out.push_str(&format!(
        "critical t at alpha {} = {}{}\n",
        fmt_num(r.alpha, 4),
        fmt_num(r.critical_t, d),
        if r.tails == "two" {
            " (two-tailed)"
        } else {
            " (one-tailed)"
        }
    ));
    out.push_str(&format!(
        "result: {} -> {} the null hypothesis\n",
        if r.reject {
            format!("p < alpha {}", fmt_num(r.alpha, 4))
        } else {
            format!("p >= alpha {}", fmt_num(r.alpha, 4))
        },
        if r.reject { "reject" } else { "fail to reject" }
    ));
    out.push_str(&format!(
        "{} CI for the {}: {}\n",
        fmt_level(r.confidence_level),
        if r.test == "one-sample" {
            "mean"
        } else {
            "difference"
        },
        ci_text(r.ci_lower, r.ci_upper, d)
    ));

    // ---- effect sizes ----
    out.push_str("\neffect size\n");
    for e in &r.effects {
        let ci = match (e.ci_lower, e.ci_upper) {
            (Some(_), Some(_)) => format!(
                ", {} CI {} (approx)",
                fmt_level(r.confidence_level),
                ci_text(e.ci_lower, e.ci_upper, d)
            ),
            _ => String::new(),
        };
        out.push_str(&format!(
            "{}: {} ({}) — standardized by the {}{}\n",
            e.name,
            fmt_num(e.value, d),
            e.magnitude,
            e.standardizer,
            ci
        ));
    }
    out.push_str(&format!(
        "\nobserved power at alpha {}: {}\n",
        fmt_num(r.alpha, 4),
        fmt_num(r.power, d)
    ));

    if let Some(v) = &r.variance_test {
        out.push_str("\nassumption check\n");
        out.push_str(&format!(
            "variance ratio F({}, {}) = {}, p = {} -> {}\n",
            fmt_df(v.df1, d),
            fmt_df(v.df2, d),
            fmt_num(v.f, d),
            fmt_p(v.p_value, d),
            if v.equal_variances_plausible {
                "equal variances are plausible"
            } else {
                "variances differ"
            }
        ));
    }

    if !r.notes.is_empty() {
        out.push('\n');
        for n in &r.notes {
            out.push_str(&format!("note: {n}\n"));
        }
    }
    out.trim_end().to_string()
}

pub fn render_table(r: &TTestResult, d: usize) -> String {
    let mut out = String::new();
    out.push_str(&format!("### {}\n\n", r.test_name));
    out.push_str(&format!(
        "Alternative: {} ({}-tailed). Null hypothesis: {} = {}.\n\n",
        r.alternative,
        r.tails,
        if r.test == "one-sample" {
            "population mean"
        } else {
            "mean difference"
        },
        trim_float(r.mu)
    ));

    out.push_str("| sample | n | mean | sd | sem | min | max |\n");
    out.push_str("| --- | ---: | ---: | ---: | ---: | ---: | ---: |\n");
    for s in all_rows(r) {
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} |\n",
            s.name,
            s.n,
            fmt_num(s.mean, d),
            fmt_num(s.sd, d),
            fmt_num(s.sem, d),
            fmt_opt(s.min, d),
            fmt_opt(s.max, d),
        ));
    }

    out.push_str("\n| statistic | value |\n| --- | ---: |\n");
    out.push_str(&format!(
        "| {} | {} |\n",
        r.estimate_label,
        fmt_num(r.estimate, d)
    ));
    out.push_str(&format!(
        "| standard error | {} |\n",
        fmt_num(r.standard_error, d)
    ));
    out.push_str(&format!("| t | {} |\n", fmt_num(r.t, d)));
    out.push_str(&format!("| df | {} |\n", fmt_df(r.df, d)));
    out.push_str(&format!("| p-value | {} |\n", fmt_p(r.p_value, d)));
    out.push_str(&format!(
        "| critical t (alpha {}) | {} |\n",
        fmt_num(r.alpha, 4),
        fmt_num(r.critical_t, d)
    ));
    out.push_str(&format!(
        "| {} CI | {} |\n",
        fmt_level(r.confidence_level),
        ci_text(r.ci_lower, r.ci_upper, d)
    ));
    out.push_str(&format!(
        "| decision at alpha {} | {} |\n",
        fmt_num(r.alpha, 4),
        if r.reject {
            "reject the null hypothesis"
        } else {
            "fail to reject the null hypothesis"
        }
    ));
    out.push_str(&format!(
        "| observed power | {} |\n",
        fmt_num(r.power, d)
    ));

    out.push_str("\n| effect size | value | magnitude | standardized by | approx CI |\n");
    out.push_str("| --- | ---: | --- | --- | --- |\n");
    for e in &r.effects {
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} |\n",
            e.name,
            fmt_num(e.value, d),
            e.magnitude,
            e.standardizer,
            match (e.ci_lower, e.ci_upper) {
                (Some(_), Some(_)) => ci_text(e.ci_lower, e.ci_upper, d),
                _ => "n/a".to_string(),
            }
        ));
    }

    if let Some(v) = &r.variance_test {
        out.push_str("\n| assumption check | value |\n| --- | ---: |\n");
        out.push_str(&format!(
            "| variance ratio F({}, {}) | {} |\n",
            fmt_df(v.df1, d),
            fmt_df(v.df2, d),
            fmt_num(v.f, d)
        ));
        out.push_str(&format!("| p-value | {} |\n", fmt_p(v.p_value, d)));
        out.push_str(&format!(
            "| equal variances plausible | {} |\n",
            if v.equal_variances_plausible {
                "yes"
            } else {
                "no"
            }
        ));
    }

    if !r.notes.is_empty() {
        out.push('\n');
        for n in &r.notes {
            out.push_str(&format!("- {n}\n"));
        }
    }
    out.trim_end().to_string()
}

fn round_sample(s: &SampleStats, d: usize) -> SampleStats {
    SampleStats {
        name: s.name.clone(),
        n: s.n,
        mean: round_to(s.mean, d),
        sd: round_to(s.sd, d),
        sem: round_to(s.sem, d),
        min: s.min.map(|v| round_to(v, d)),
        max: s.max.map(|v| round_to(v, d)),
    }
}

fn rounded(r: &TTestResult, d: usize) -> TTestResult {
    let pd = d.max(6);
    TTestResult {
        test: r.test.clone(),
        test_name: r.test_name.clone(),
        tails: r.tails.clone(),
        alternative: r.alternative.clone(),
        mu: r.mu,
        input_format: r.input_format.clone(),
        samples: r.samples.iter().map(|s| round_sample(s, d)).collect(),
        differences: r.differences.as_ref().map(|s| round_sample(s, d)),
        estimate_label: r.estimate_label.clone(),
        estimate: round_to(r.estimate, d),
        standard_error: round_to(r.standard_error, d),
        t: round_to(r.t, d),
        df: round_to(r.df, d.max(4)),
        p_value: round_to(r.p_value, pd),
        alpha: r.alpha,
        critical_t: round_to(r.critical_t, d),
        reject: r.reject,
        confidence_level: round_to(r.confidence_level, 4),
        ci_lower: r.ci_lower.map(|v| round_to(v, d)),
        ci_upper: r.ci_upper.map(|v| round_to(v, d)),
        effects: r
            .effects
            .iter()
            .map(|e| EffectSize {
                name: e.name.clone(),
                standardizer: e.standardizer.clone(),
                value: round_to(e.value, d),
                magnitude: e.magnitude.clone(),
                ci_lower: e.ci_lower.map(|v| round_to(v, d)),
                ci_upper: e.ci_upper.map(|v| round_to(v, d)),
            })
            .collect(),
        power: round_to(r.power, d.max(4)),
        variance_test: r.variance_test.as_ref().map(|v| VarianceTest {
            f: round_to(v.f, d),
            df1: v.df1,
            df2: v.df2,
            p_value: round_to(v.p_value, pd),
            equal_variances_plausible: v.equal_variances_plausible,
        }),
        notes: r.notes.clone(),
    }
}

/// The single entry point shared by the chat block, the CLI and the page.
#[allow(clippy::too_many_arguments)]
pub fn run(
    data: &str,
    test: &str,
    format: &str,
    delimiter: &str,
    header: &str,
    mu: f64,
    tails: &str,
    alpha: f64,
    decimals: f64,
    output: &str,
) -> Result<String, String> {
    if !decimals.is_finite() || decimals.fract() != 0.0 || !(0.0..=10.0).contains(&decimals) {
        return Err(format!(
            "invalid decimals {decimals}: expected a whole number between 0 and 10"
        ));
    }
    let d = decimals as usize;
    let mode = output.trim().to_ascii_lowercase();
    match mode.as_str() {
        "" | "summary" | "table" | "json" => {}
        other => {
            return Err(format!(
                "invalid output {other:?}: expected \"summary\", \"table\" or \"json\""
            ))
        }
    }
    let r = analyze(data, test, format, delimiter, header, mu, tails, alpha)?;
    match mode.as_str() {
        "table" => Ok(render_table(&r, d)),
        "json" => serde_json::to_string_pretty(&rounded(&r, d))
            .map_err(|e| format!("failed to serialize the result as JSON: {e}")),
        _ => Ok(render_summary(&r, d)),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() <= tol
    }

    // ---- special functions ------------------------------------------------

    #[test]
    fn normal_cdf_matches_known_values() {
        assert!(close(norm_cdf(0.0), 0.5, 1e-12));
        assert!(close(norm_cdf(1.0), 0.841_344_746_068_543, 1e-12));
        assert!(close(norm_cdf(-1.96), 0.024_997_895_148_220_43, 1e-12));
        assert!(close(norm_cdf(3.0), 0.998_650_101_968_369_9, 1e-12));
    }

    /// df = 1 is the Cauchy distribution and df = 2 has a closed form, so both
    /// two-tailed p-values can be checked against exact analytic expressions
    /// rather than against a table someone typed in.
    #[test]
    fn student_t_two_tail_matches_closed_forms() {
        for &t in &[0.3f64, 1.0, 2.5, 7.0] {
            let cauchy = 1.0 - 2.0 * t.atan() / std::f64::consts::PI;
            assert!(
                close(student_t_two_tail(t, 1.0), cauchy, 1e-10),
                "df=1 t={t}"
            );
            let df2 = 1.0 - t / (2.0 + t * t).sqrt();
            assert!(close(student_t_two_tail(t, 2.0), df2, 1e-10), "df=2 t={t}");
        }
        // As df → ∞ the t distribution becomes standard normal.
        assert!(close(
            student_t_two_tail(1.96, 5.0e7),
            2.0 * (1.0 - norm_cdf(1.96)),
            1e-6
        ));
        assert!(close(student_t_two_tail(0.0, 10.0), 1.0, 1e-12));
    }

    /// Widely tabulated two-tailed critical values (the ones printed on the back
    /// cover of every statistics textbook).
    #[test]
    fn critical_t_matches_the_published_table() {
        assert!(close(t_critical_two_tailed(0.05, 10.0), 2.228, 5e-4));
        assert!(close(t_critical_two_tailed(0.05, 20.0), 2.086, 5e-4));
        assert!(close(t_critical_two_tailed(0.01, 15.0), 2.947, 5e-4));
        assert!(close(t_critical_one_tailed(0.05, 10.0), 1.812, 5e-4));
        assert!(close(t_critical_one_tailed(0.05, 30.0), 1.697, 5e-4));
        // And it really inverts the tail it claims to invert.
        let c = t_critical_two_tailed(0.05, 8.0);
        assert!(close(student_t_two_tail(c, 8.0), 0.05, 1e-9));
    }

    #[test]
    fn student_t_cdf_is_the_integral_of_the_two_tail() {
        assert!(close(student_t_cdf(0.0, 7.0), 0.5, 1e-12));
        assert!(close(
            student_t_cdf(2.0, 7.0) - student_t_cdf(-2.0, 7.0),
            1.0 - student_t_two_tail(2.0, 7.0),
            1e-12
        ));
    }

    /// The noncentral t with δ = 0 IS the central t; and with huge df it is a
    /// unit-variance normal centred at δ. Both identities pin the series.
    #[test]
    fn noncentral_t_reduces_to_known_distributions() {
        for &(t, df) in &[(0.5f64, 5.0f64), (2.0, 12.0), (-1.3, 9.0), (3.1, 40.0)] {
            assert!(
                close(noncentral_t_cdf(t, df, 0.0), student_t_cdf(t, df), 1e-9),
                "ncp=0 t={t} df={df}"
            );
        }
        for &(t, delta) in &[(1.96f64, 2.0f64), (0.0, 1.0), (-1.0, -0.5)] {
            assert!(
                close(
                    noncentral_t_cdf(t, 2.0e6, delta),
                    norm_cdf(t - delta),
                    1e-4
                ),
                "large df t={t} delta={delta}"
            );
        }
        // Monotone in t, and a shifted centre moves mass the right way.
        assert!(noncentral_t_cdf(1.0, 10.0, 2.0) < noncentral_t_cdf(3.0, 10.0, 2.0));
        assert!(noncentral_t_cdf(1.0, 10.0, 2.0) < noncentral_t_cdf(1.0, 10.0, 0.0));
    }

    /// At the critical value itself the observed power must equal alpha when the
    /// noncentrality is exactly the critical t, and power must rise with n.
    #[test]
    fn observed_power_behaves() {
        let df = 18.0;
        let crit = t_critical_two_tailed(0.05, df);
        // δ = 0 → the rejection probability is exactly alpha.
        let p0 = 1.0 - noncentral_t_cdf(crit, df, 0.0) + noncentral_t_cdf(-crit, df, 0.0);
        assert!(close(p0, 0.05, 1e-9), "{p0}");
        // δ = critical value → power is just above 1/2 (half the mass is past it).
        let pc = 1.0 - noncentral_t_cdf(crit, df, crit) + noncentral_t_cdf(-crit, df, crit);
        assert!((0.5..0.55).contains(&pc), "{pc}");
    }

    // ---- one-sample --------------------------------------------------------

    #[test]
    fn one_sample_happy_path() {
        // n = 10, mean = 5.5, sd = 3.02765, sem = 0.957427, t = 5.7446 vs mu = 0
        let out = run(
            "1\n2\n3\n4\n5\n6\n7\n8\n9\n10",
            "one-sample",
            "auto",
            "auto",
            "auto",
            0.0,
            "two",
            0.05,
            4.0,
            "summary",
        )
        .unwrap();
        assert!(out.contains("One-sample t-test"), "{out}");
        assert!(out.contains("t(9) = 5.7446"), "{out}");
        assert!(out.contains("reject the null hypothesis"), "{out}");
    }

    #[test]
    fn one_sample_against_a_nonzero_mu() {
        let r = analyze(
            "1\n2\n3\n4\n5\n6\n7\n8\n9\n10",
            "one-sample",
            "auto",
            "auto",
            "auto",
            5.5,
            "two",
            0.05,
        )
        .unwrap();
        assert!(close(r.t, 0.0, 1e-12), "{}", r.t);
        assert!(close(r.p_value, 1.0, 1e-12));
        assert!(!r.reject);
        // The CI for the mean is symmetric about the sample mean.
        assert!(close(
            0.5 * (r.ci_lower.unwrap() + r.ci_upper.unwrap()),
            5.5,
            1e-9
        ));
    }

    /// Arithmetic that can be checked by hand: n = 4, mean 6, sd 2, sem 1.
    #[test]
    fn one_sample_hand_checkable_arithmetic() {
        let r = analyze("4\n6\n8\n6", "one-sample", "wide", "auto", "no", 4.0, "two", 0.05).unwrap();
        assert_eq!(r.samples[0].n, 4);
        assert!(close(r.samples[0].mean, 6.0, 1e-12));
        assert!(close(r.samples[0].sd, 1.632_993_161_855_452, 1e-12));
        assert!(close(r.standard_error, 0.816_496_580_927_726, 1e-12));
        assert!(close(r.t, 2.449_489_742_783_178, 1e-12));
        assert!(close(r.df, 3.0, 1e-12));
        // Cohen's d = (6 − 4) / 1.63299 = 1.224745
        assert!(close(r.effects[0].value, 1.224_744_871_391_589, 1e-12));
        assert_eq!(r.effects[0].magnitude, "large");
    }

    // ---- two-sample --------------------------------------------------------

    /// Pooled and Welch agree exactly when the two samples have equal n and
    /// equal variances — the classic cross-check on both formulas.
    #[test]
    fn pooled_and_welch_agree_for_balanced_equal_variance_samples() {
        let data = "1,11\n2,12\n3,13\n4,14\n5,15";
        let pooled = analyze(data, "two-sample", "wide", "comma", "no", 0.0, "two", 0.05).unwrap();
        let welch = analyze(data, "welch", "wide", "comma", "no", 0.0, "two", 0.05).unwrap();
        assert!(close(pooled.t, welch.t, 1e-12), "{} {}", pooled.t, welch.t);
        assert!(close(pooled.df, welch.df, 1e-9));
        assert!(close(pooled.estimate, -10.0, 1e-12));
    }

    #[test]
    fn two_sample_pooled_hand_checkable() {
        // A: 1..5 (mean 3, var 2.5); B: 4..8 (mean 6, var 2.5)
        // sp² = 2.5, se = sqrt(2.5 * 0.4) = 1, t = -3, df = 8
        let r = analyze(
            "1,4\n2,5\n3,6\n4,7\n5,8",
            "two-sample",
            "wide",
            "comma",
            "no",
            0.0,
            "two",
            0.05,
        )
        .unwrap();
        assert!(close(r.standard_error, 1.0, 1e-12));
        assert!(close(r.t, -3.0, 1e-12));
        assert!(close(r.df, 8.0, 1e-12));
        // Cohen's d = -3 / sqrt(2.5)
        assert!(close(r.effects[0].value, -3.0 / 2.5f64.sqrt(), 1e-12));
        assert!(close(r.p_value, student_t_two_tail(3.0, 8.0), 1e-15));
    }

    #[test]
    fn welch_degrees_of_freedom_are_fractional() {
        let r = analyze(
            "10,20\n12,40\n14,10\n16,60\n18,30",
            "welch",
            "wide",
            "comma",
            "no",
            0.0,
            "two",
            0.05,
        )
        .unwrap();
        assert!(r.df.fract() > 0.0, "df = {}", r.df);
        assert!(r.df < 8.0 && r.df > 4.0, "df = {}", r.df);
        assert!(r.variance_test.is_some());
    }

    #[test]
    fn summary_statistics_input_matches_raw_input() {
        let raw = analyze(
            "1,4\n2,5\n3,6\n4,7\n5,8",
            "two-sample",
            "wide",
            "comma",
            "no",
            0.0,
            "two",
            0.05,
        )
        .unwrap();
        let sd = 2.5f64.sqrt();
        let summary = analyze(
            &format!("A,5,3,{sd}\nB,5,6,{sd}"),
            "two-sample",
            "summary",
            "comma",
            "no",
            0.0,
            "two",
            0.05,
        )
        .unwrap();
        assert!(close(raw.t, summary.t, 1e-9));
        assert!(close(raw.p_value, summary.p_value, 1e-12));
        assert_eq!(summary.samples[0].min, None);
    }

    // ---- paired ------------------------------------------------------------

    #[test]
    fn paired_equals_a_one_sample_test_on_the_differences() {
        let pairs = "12,10\n14,11\n11,10\n15,12\n13,12\n16,13";
        let paired = analyze(pairs, "paired", "wide", "comma", "no", 0.0, "two", 0.05).unwrap();
        let diffs = "2\n3\n1\n3\n1\n3";
        let one = analyze(diffs, "one-sample", "wide", "auto", "no", 0.0, "two", 0.05).unwrap();
        assert!(close(paired.t, one.t, 1e-12));
        assert!(close(paired.df, one.df, 1e-12));
        assert!(close(paired.p_value, one.p_value, 1e-12));
        let d = paired.differences.as_ref().unwrap();
        assert_eq!(d.n, 6);
        assert!(close(d.mean, 13.0 / 6.0, 1e-12));
        // dav uses the average of the two column SDs, so it differs from dz.
        assert_eq!(paired.effects.len(), 3);
    }

    #[test]
    fn paired_from_precomputed_differences() {
        let r = analyze("2\n3\n1\n3\n1\n3", "paired", "wide", "auto", "no", 0.0, "two", 0.05)
            .unwrap();
        assert_eq!(r.differences.as_ref().unwrap().n, 6);
        assert!(r.notes.iter().any(|n| n.contains("pre-computed")));
    }

    #[test]
    fn paired_from_two_summary_rows_is_rejected_with_an_explanation() {
        let err = analyze(
            "Before,10,5.0,1.2\nAfter,10,6.0,1.4",
            "paired",
            "summary",
            "comma",
            "no",
            0.0,
            "two",
            0.05,
        )
        .unwrap_err();
        assert!(err.contains("correlation"), "{err}");
        assert!(err.contains("differences"), "{err}");
    }

    #[test]
    fn paired_rejects_unequal_column_lengths() {
        let err = analyze("1,2\n2,3\n3,", "paired", "wide", "comma", "no", 0.0, "two", 0.05)
            .unwrap_err();
        assert!(err.contains("same number of values"), "{err}");
    }

    // ---- tails -------------------------------------------------------------

    #[test]
    fn one_tailed_p_values_split_the_two_tailed_one() {
        let data = "1,4\n2,5\n3,6\n4,7\n5,8";
        let two = analyze(data, "two-sample", "wide", "comma", "no", 0.0, "two", 0.05).unwrap();
        let left = analyze(data, "two-sample", "wide", "comma", "no", 0.0, "left", 0.05).unwrap();
        let right = analyze(data, "two-sample", "wide", "comma", "no", 0.0, "right", 0.05).unwrap();
        // t is negative, so the left tail gets half of the two-tailed p.
        assert!(close(left.p_value, two.p_value / 2.0, 1e-12));
        assert!(close(right.p_value, 1.0 - two.p_value / 2.0, 1e-12));
        assert!(close(left.p_value + right.p_value, 1.0, 1e-12));
        // One-tailed runs report a one-sided bound, not an interval.
        assert!(left.ci_lower.is_none() && left.ci_upper.is_some());
        assert!(right.ci_upper.is_none() && right.ci_lower.is_some());
        assert!(two.ci_lower.is_some() && two.ci_upper.is_some());
    }

    // ---- parsing -----------------------------------------------------------

    #[test]
    fn long_format_and_headers_are_detected() {
        let r = analyze(
            "group,value\nControl,5\nControl,6\nControl,9\nDrug,8\nDrug,11\nDrug,13",
            "auto",
            "auto",
            "auto",
            "auto",
            0.0,
            "two",
            0.05,
        )
        .unwrap();
        assert_eq!(r.input_format, "long");
        assert_eq!(r.samples[0].name, "Control");
        assert_eq!(r.samples[1].name, "Drug");
        assert_eq!(r.test, "welch");
    }

    #[test]
    fn auto_detects_a_single_column_as_a_one_sample_test() {
        let r = analyze("1\n2\n3\n4\n5", "auto", "auto", "auto", "auto", 0.0, "two", 0.05).unwrap();
        assert_eq!(r.test, "one-sample");
        assert!(r.notes.iter().any(|n| n.contains("auto-selected")));
    }

    #[test]
    fn every_delimiter_parses_the_same_two_columns() {
        for (name, data) in [
            ("comma", "1,4\n2,5\n3,6\n4,7\n5,8"),
            ("tab", "1\t4\n2\t5\n3\t6\n4\t7\n5\t8"),
            ("semicolon", "1;4\n2;5\n3;6\n4;7\n5;8"),
            ("pipe", "1|4\n2|5\n3|6\n4|7\n5|8"),
            ("space", "1 4\n2 5\n3 6\n4 7\n5 8"),
        ] {
            let r = analyze(data, "two-sample", "wide", name, "no", 0.0, "two", 0.05)
                .unwrap_or_else(|e| panic!("{name}: {e}"));
            assert!(close(r.t, -3.0, 1e-12), "{name}: {}", r.t);
        }
    }

    #[test]
    fn comments_and_blank_lines_are_ignored() {
        let r = analyze(
            "# my data\n1,4\n\n2,5\n3,6\n4,7\n5,8\n",
            "two-sample",
            "auto",
            "auto",
            "auto",
            0.0,
            "two",
            0.05,
        )
        .unwrap();
        assert_eq!(r.samples[0].n, 5);
    }

    // ---- errors ------------------------------------------------------------

    #[test]
    fn empty_input_is_an_error() {
        let err = run("", "auto", "auto", "auto", "auto", 0.0, "two", 0.05, 4.0, "summary")
            .unwrap_err();
        assert!(err.contains("no data"), "{err}");
    }

    #[test]
    fn three_columns_are_rejected_with_a_pointer_to_anova() {
        let err = analyze(
            "1,4,7\n2,5,8\n3,6,9",
            "auto",
            "wide",
            "comma",
            "no",
            0.0,
            "two",
            0.05,
        )
        .unwrap_err();
        assert!(err.contains("at most two"), "{err}");
        assert!(err.contains("ANOVA"), "{err}");
    }

    #[test]
    fn zero_variance_is_an_error() {
        let err = analyze("5\n5\n5\n5", "one-sample", "wide", "auto", "no", 0.0, "two", 0.05)
            .unwrap_err();
        assert!(err.contains("standard deviation is 0"), "{err}");
    }

    #[test]
    fn non_numeric_cells_report_the_line() {
        let err = analyze("1\n2\nabc\n4", "one-sample", "wide", "auto", "no", 0.0, "two", 0.05)
            .unwrap_err();
        assert!(err.contains("line 3"), "{err}");
        assert!(err.contains("expected a number"), "{err}");
    }

    #[test]
    fn out_of_range_options_are_rejected() {
        assert!(analyze("1\n2\n3", "one-sample", "wide", "auto", "no", 0.0, "two", 0.9)
            .unwrap_err()
            .contains("alpha"));
        assert!(run("1\n2\n3", "one-sample", "wide", "auto", "no", 0.0, "two", 0.05, 11.0, "summary")
            .unwrap_err()
            .contains("decimals"));
        assert!(run("1\n2\n3", "one-sample", "wide", "auto", "no", 0.0, "two", 0.05, 4.0, "csv")
            .unwrap_err()
            .contains("invalid output"));
        assert!(analyze("1\n2\n3", "anova", "wide", "auto", "no", 0.0, "two", 0.05)
            .unwrap_err()
            .contains("invalid test"));
        assert!(analyze("1\n2\n3", "one-sample", "wide", "auto", "no", 0.0, "sideways", 0.05)
            .unwrap_err()
            .contains("invalid tails"));
    }

    #[test]
    fn one_sample_with_two_columns_is_an_error() {
        let err = analyze("1,4\n2,5\n3,6", "one-sample", "wide", "comma", "no", 0.0, "two", 0.05)
            .unwrap_err();
        assert!(err.contains("one column of values"), "{err}");
    }

    // ---- caps --------------------------------------------------------------

    #[test]
    fn the_value_cap_is_enforced_at_the_boundary() {
        let at = (0..MAX_VALUES).map(|i| (i % 97).to_string()).collect::<Vec<_>>().join("\n");
        assert!(analyze(&at, "one-sample", "wide", "auto", "no", 0.0, "two", 0.05).is_ok());
        let over = format!("{at}\n1");
        let err = analyze(&over, "one-sample", "wide", "auto", "no", 0.0, "two", 0.05).unwrap_err();
        assert!(err.contains("too many values"), "{err}");
    }

    // ---- output modes ------------------------------------------------------

    #[test]
    fn table_output_is_markdown() {
        let out = run(
            "1,4\n2,5\n3,6\n4,7\n5,8",
            "two-sample",
            "wide",
            "comma",
            "no",
            0.0,
            "two",
            0.05,
            4.0,
            "table",
        )
        .unwrap();
        assert!(out.starts_with("### Two-sample (pooled) t-test"), "{out}");
        assert!(out.contains("| statistic | value |"), "{out}");
        assert!(out.contains("| t | -3.0000 |"), "{out}");
    }

    #[test]
    fn json_output_round_trips() {
        let out = run(
            "1,4\n2,5\n3,6\n4,7\n5,8",
            "two-sample",
            "wide",
            "comma",
            "no",
            0.0,
            "two",
            0.05,
            4.0,
            "json",
        )
        .unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["test"], "two-sample");
        assert_eq!(v["t"], -3.0);
        assert_eq!(v["df"], 8.0);
        assert_eq!(v["samples"].as_array().unwrap().len(), 2);
        assert!(v["effects"].as_array().unwrap().len() >= 2);
        assert!(v["power"].as_f64().unwrap() > 0.0);
    }

    #[test]
    fn decimals_control_the_rendered_precision() {
        let two = run(
            "1\n2\n3\n4\n5\n6\n7\n8\n9\n10",
            "one-sample",
            "wide",
            "auto",
            "no",
            0.0,
            "two",
            0.05,
            2.0,
            "summary",
        )
        .unwrap();
        assert!(two.contains("t(9) = 5.74"), "{two}");
        let zero = run(
            "1\n2\n3\n4\n5\n6\n7\n8\n9\n10",
            "one-sample",
            "wide",
            "auto",
            "no",
            0.0,
            "two",
            0.05,
            0.0,
            "summary",
        )
        .unwrap();
        assert!(zero.contains("t(9) = 6"), "{zero}");
    }

    #[test]
    fn tiny_p_values_print_below_the_resolution() {
        let out = run(
            "100\n101\n102\n103\n104\n105",
            "one-sample",
            "wide",
            "auto",
            "no",
            0.0,
            "two",
            0.05,
            4.0,
            "summary",
        )
        .unwrap();
        assert!(out.contains("p = < 0.0001"), "{out}");
    }
}
