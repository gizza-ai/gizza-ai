//! gizza-ai/anova core — one-way analysis of variance (ANOVA).
//!
//! Takes raw observations (long `group,value` rows or wide one-column-per-group
//! data) or per-group summary statistics (`name,n,mean,sd`) and produces the
//! full one-way ANOVA table: per-group descriptive statistics, between/within/
//! total sums of squares, degrees of freedom, mean squares, the F statistic and
//! its right-tail p-value, the critical F at the chosen significance level,
//! eta-squared / omega-squared effect sizes, Welch's unequal-variance ANOVA,
//! Levene's (Brown–Forsythe, median-centred) homogeneity-of-variance test and
//! optional pairwise post-hoc comparisons (Tukey HSD, Games–Howell, Fisher's
//! LSD, Bonferroni, Holm).
//!
//! Everything is pure Rust with no numeric dependencies, so the same code runs
//! natively (CLI), in the wasm32-wasip1 chat block and in the browser page. The
//! special functions (log-gamma, regularized incomplete gamma/beta, the normal
//! CDF and the studentized-range distribution) are implemented and unit-tested
//! here — see the `special` tests at the bottom.

use serde::Serialize;

/// Hard cap on the number of parsed observations (keeps a pasted spreadsheet
/// from locking up the browser tab).
pub const MAX_VALUES: usize = 200_000;
/// Hard cap on the number of groups.
pub const MAX_GROUPS: usize = 1_000;

// ---------------------------------------------------------------------------
// Result types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GroupStats {
    pub name: String,
    /// Number of observations in the group.
    pub n: usize,
    pub mean: f64,
    /// Sample standard deviation (n − 1 denominator); 0 for a single observation.
    pub sd: f64,
    /// Sample variance (n − 1 denominator); 0 for a single observation.
    pub variance: f64,
    /// Standard error of the mean (sd / √n).
    pub sem: f64,
    /// Sum of the observations — absent for summary-statistics input.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sum: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PairComparison {
    pub group_a: String,
    pub group_b: String,
    /// mean(a) − mean(b).
    pub mean_difference: f64,
    /// Standard error of the difference.
    pub std_error: f64,
    /// Tukey's q for `tukey`, Student's t for the t-based methods.
    pub statistic: f64,
    /// Unadjusted p-value for the pair.
    pub p_value: f64,
    /// p-value after the multiple-comparison adjustment (identical to
    /// `p_value` for `tukey` and `lsd`, which adjust the reference
    /// distribution rather than the p-value).
    pub p_adjusted: f64,
    /// Family-wise confidence interval for the difference; absent for `holm`,
    /// which has no closed-form interval.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ci_lower: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ci_upper: Option<f64>,
    pub significant: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AnovaResult {
    /// "one-way ANOVA".
    pub test: String,
    /// Which input shape was parsed: "long", "wide" or "summary".
    pub input_format: String,
    pub groups: Vec<GroupStats>,
    /// Number of groups (k).
    pub group_count: usize,
    /// Total number of observations (N).
    pub observations: usize,
    pub grand_mean: f64,
    pub ss_between: f64,
    pub df_between: usize,
    pub ms_between: f64,
    pub ss_within: f64,
    pub df_within: usize,
    pub ms_within: f64,
    pub ss_total: f64,
    pub df_total: usize,
    pub f_statistic: f64,
    pub p_value: f64,
    /// Right-tail critical F at `alpha` for (df_between, df_within).
    pub f_critical: f64,
    pub alpha: f64,
    /// True when p < alpha (at least one group mean differs).
    pub reject_null: bool,
    /// Proportion of total variance explained: SS_between / SS_total.
    pub eta_squared: f64,
    /// Less biased effect size; can be slightly negative for tiny effects.
    pub omega_squared: f64,
    /// Welch's F for unequal variances — absent when a group has n < 2 or zero variance.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub welch_f: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub welch_df1: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub welch_df2: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub welch_p_value: Option<f64>,
    /// Levene's test (Brown–Forsythe variant: deviations from the group median)
    /// — needs raw observations, so absent for summary-statistics input.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub levene_f: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub levene_df1: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub levene_df2: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub levene_p_value: Option<f64>,
    /// "none", "tukey", "games-howell", "lsd", "bonferroni" or "holm".
    pub posthoc: String,
    pub comparisons: Vec<PairComparison>,
    /// Human-readable caveats (skipped sub-tests, unbalanced design, …).
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
        if x * x < 0.5 + 1.0 {
            1.0 - gamma_p_series(0.5, x * x)
        } else {
            gamma_q_cf(0.5, x * x)
        }
    } else {
        1.0 + gamma_p(0.5, x * x)
    }
}

/// Standard normal CDF Φ(z).
fn norm_cdf(z: f64) -> f64 {
    0.5 * erfc(-z / std::f64::consts::SQRT_2)
}

/// Standard normal PDF φ(z).
fn norm_pdf(z: f64) -> f64 {
    (-0.5 * z * z).exp() / (2.0 * std::f64::consts::PI).sqrt()
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

/// Critical F: the value with right-tail probability `alpha`.
pub fn f_critical(alpha: f64, d1: f64, d2: f64) -> f64 {
    invert_upper_tail(|x| f_upper_tail(x, d1, d2), alpha, 1.0e7)
}

/// Two-tailed critical t: the value with P(|T| ≥ t) = `alpha`.
pub fn t_critical(alpha: f64, df: f64) -> f64 {
    invert_upper_tail(|x| student_t_two_tail(x, df), alpha, 1.0e7)
}

// ---- studentized range (Tukey) --------------------------------------------

/// 16-point Gauss–Legendre nodes/weights on [-1, 1] (symmetric halves).
const GL_X: [f64; 8] = [
    0.095_012_509_837_637_44,
    0.281_603_550_779_258_9,
    0.458_016_777_657_227_4,
    0.617_876_244_402_643_7,
    0.755_404_408_355_003,
    0.865_631_202_387_831_7,
    0.944_575_023_073_232_6,
    0.989_400_934_991_649_9,
];
const GL_W: [f64; 8] = [
    0.189_450_610_455_068_5,
    0.182_603_415_044_923_6,
    0.169_156_519_395_002_5,
    0.149_595_988_816_576_7,
    0.124_628_971_255_533_9,
    0.095_158_511_682_492_78,
    0.062_253_523_938_647_9,
    0.027_152_459_411_754_09,
];

/// Composite 16-point Gauss–Legendre integration of `f` over [a, b] in `panels` panels.
fn gauss_legendre<F: Fn(f64) -> f64>(f: F, a: f64, b: f64, panels: usize) -> f64 {
    if !(b > a) {
        return 0.0;
    }
    let h = (b - a) / panels as f64;
    let mut total = 0.0;
    for p in 0..panels {
        let lo = a + h * p as f64;
        let c = lo + 0.5 * h;
        let r = 0.5 * h;
        let mut sum = 0.0;
        for i in 0..8 {
            sum += GL_W[i] * (f(c + r * GL_X[i]) + f(c - r * GL_X[i]));
        }
        total += r * sum;
    }
    total
}

/// P(range of `k` iid standard normals < u) = k ∫ φ(z) [Φ(z) − Φ(z−u)]^(k−1) dz.
fn wprob(u: f64, k: f64) -> f64 {
    if u <= 0.0 {
        return 0.0;
    }
    let p = gauss_legendre(
        |z| norm_pdf(z) * (norm_cdf(z) - norm_cdf(z - u)).max(0.0).powf(k - 1.0),
        -8.5,
        8.5,
        24,
    ) * k;
    p.clamp(0.0, 1.0)
}

/// CDF of the studentized range: P(Q < q) for `k` groups and `df` error degrees
/// of freedom. Averages `wprob` over the sampling distribution of the error
/// scale s = √(χ²_df / df).
fn ptukey(q: f64, k: f64, df: f64) -> f64 {
    if q <= 0.0 {
        return 0.0;
    }
    if !q.is_finite() {
        return 1.0;
    }
    if df > 25_000.0 {
        return wprob(q, k);
    }
    // Wilson–Hilferty bounds on χ²_df at ±8.5 sd, converted to the s scale.
    let a = 2.0 / (9.0 * df);
    let cube = |z: f64| {
        let t = 1.0 - a + z * a.sqrt();
        if t <= 0.0 {
            0.0
        } else {
            t * t * t
        }
    };
    let s_lo = cube(-8.5).max(0.0).sqrt();
    let s_hi = (cube(8.5) + 1e-9).sqrt().max(s_lo + 1e-6);
    // ln of the density of s = √(χ²_df/df).
    let half = df / 2.0;
    let ln_c = half * df.ln() - (half - 1.0) * std::f64::consts::LN_2 - ln_gamma(half);
    let dens = move |s: f64| {
        if s <= 0.0 {
            0.0
        } else {
            (ln_c + (df - 1.0) * s.ln() - df * s * s / 2.0).exp()
        }
    };
    let p = gauss_legendre(|s| dens(s) * wprob(q * s, k), s_lo, s_hi, 24);
    p.clamp(0.0, 1.0)
}

/// Right-tail studentized-range p-value: P(Q ≥ q).
pub fn tukey_upper_tail(q: f64, k: f64, df: f64) -> f64 {
    (1.0 - ptukey(q, k, df)).clamp(0.0, 1.0)
}

/// Critical studentized range q with right-tail probability `alpha`.
pub fn tukey_critical(alpha: f64, k: f64, df: f64) -> f64 {
    invert_upper_tail(|x| tukey_upper_tail(x, k, df), alpha, 200.0)
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

/// Parsed input: either raw observations per group, or per-group summaries.
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

/// Decide long vs wide when `format = "auto"`.
fn detect_format(rows: &[Vec<String>], skipped_header: bool) -> &'static str {
    let body = if skipped_header { &rows[1..] } else { rows };
    if body.is_empty() {
        return "wide";
    }
    if !body.iter().all(|r| r.len() == 2) {
        return "wide";
    }
    let label_first = body.iter().filter(|r| !is_number(&r[0])).count();
    let label_second = body.iter().filter(|r| !is_number(&r[1])).count();
    if label_first > 0 && label_first >= label_second {
        "long"
    } else if label_second > 0 {
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
            "no data: paste observations as \"group,value\" rows (long) or one column per group (wide)"
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
                "invalid format {other:?}: expected \"auto\", \"long\", \"wide\" or \"summary\""
            ))
        }
    }

    // A header row is one whose numeric cells don't parse where numbers are expected.
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
    order: &mut Vec<String>,
    name: &str,
    value: f64,
) -> Result<(), String> {
    match groups.iter_mut().find(|(n, _)| n == name) {
        Some((_, vals)) => vals.push(value),
        None => {
            if groups.len() >= MAX_GROUPS {
                return Err(format!("too many groups: the maximum is {MAX_GROUPS}"));
            }
            order.push(name.to_string());
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
    let mut order: Vec<String> = Vec::new();
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
        push_value(&mut groups, &mut order, label, value)?;
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
    if width > MAX_GROUPS {
        return Err(format!("too many groups: the maximum is {MAX_GROUPS}"));
    }
    let names: Vec<String> = (0..width)
        .map(|c| {
            let from_header = if has_header {
                rows[0].get(c).map(|s| s.trim()).unwrap_or("")
            } else {
                ""
            };
            if from_header.is_empty() {
                format!("Group {}", c + 1)
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
    let mut out = Vec::new();
    for (i, row) in body.iter().enumerate() {
        let line_no = start + i + 1;
        if row.len() < 4 {
            return Err(format!(
                "line {line_no}: expected 4 fields (name, n, mean, sd), got {} — summary format needs one \"name,n,mean,sd\" row per group",
                row.len()
            ));
        }
        let name = if row[0].is_empty() {
            format!("Group {}", i + 1)
        } else {
            row[0].clone()
        };
        let n_f = parse_number(&row[1], line_no)?;
        if n_f < 1.0 || n_f.fract() != 0.0 {
            return Err(format!(
                "line {line_no}: n must be a whole number ≥ 1, got {}",
                row[1]
            ));
        }
        let mean = parse_number(&row[2], line_no)?;
        let sd = parse_number(&row[3], line_no)?;
        if sd < 0.0 {
            return Err(format!(
                "line {line_no}: the standard deviation must be ≥ 0, got {sd}"
            ));
        }
        if out.len() >= MAX_GROUPS {
            return Err(format!("too many groups: the maximum is {MAX_GROUPS}"));
        }
        out.push((name, n_f as usize, mean, sd));
    }
    Ok(Parsed::Summary(out))
}

// ---------------------------------------------------------------------------
// The ANOVA itself
// ---------------------------------------------------------------------------

fn mean_of(v: &[f64]) -> f64 {
    v.iter().sum::<f64>() / v.len() as f64
}

fn median_of(v: &[f64]) -> f64 {
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = s.len();
    if n % 2 == 1 {
        s[n / 2]
    } else {
        0.5 * (s[n / 2 - 1] + s[n / 2])
    }
}

/// Between/within sums of squares for a set of raw groups.
fn ss_from_groups(groups: &[Vec<f64>]) -> (f64, f64, usize) {
    let n_total: usize = groups.iter().map(|g| g.len()).sum();
    let grand = groups.iter().flatten().sum::<f64>() / n_total as f64;
    let mut ssb = 0.0;
    let mut ssw = 0.0;
    for g in groups {
        let m = mean_of(g);
        ssb += g.len() as f64 * (m - grand) * (m - grand);
        for v in g {
            ssw += (v - m) * (v - m);
        }
    }
    (ssb, ssw, n_total)
}

fn round_to(v: f64, decimals: usize) -> f64 {
    let f = 10f64.powi(decimals as i32);
    (v * f).round() / f
}

/// Parse + validate the options, then run the analysis.
pub fn analyze(
    data: &str,
    format: &str,
    delimiter: &str,
    header: &str,
    alpha: f64,
    posthoc: &str,
) -> Result<AnovaResult, String> {
    let alpha = if alpha == 0.0 { 0.05 } else { alpha };
    if !alpha.is_finite() || !(0.0001..=0.5).contains(&alpha) {
        return Err(format!(
            "invalid alpha {alpha}: expected a significance level between 0.0001 and 0.5 (e.g. 0.05)"
        ));
    }
    let method = match posthoc.trim().to_ascii_lowercase().as_str() {
        "" | "none" => "none",
        "tukey" | "tukey-hsd" | "hsd" => "tukey",
        "games-howell" | "games_howell" | "gameshowell" | "gh" => "games-howell",
        "lsd" | "fisher" | "fisher-lsd" => "lsd",
        "bonferroni" => "bonferroni",
        "holm" => "holm",
        other => {
            return Err(format!(
                "invalid posthoc {other:?}: expected \"none\", \"tukey\", \"games-howell\", \"lsd\", \"bonferroni\" or \"holm\""
            ))
        }
    }
    .to_string();

    let (parsed, input_format) = parse_input(data, format, delimiter, header)?;
    let mut notes: Vec<String> = Vec::new();

    // Per-group statistics (+ raw values when we have them).
    let (stats, raw): (Vec<GroupStats>, Option<Vec<Vec<f64>>>) = match &parsed {
        Parsed::Raw(groups) => {
            let mut stats = Vec::with_capacity(groups.len());
            let mut raw = Vec::with_capacity(groups.len());
            for (name, values) in groups {
                let n = values.len();
                let mean = mean_of(values);
                let variance = if n > 1 {
                    values.iter().map(|v| (v - mean) * (v - mean)).sum::<f64>() / (n as f64 - 1.0)
                } else {
                    0.0
                };
                let sd = variance.sqrt();
                stats.push(GroupStats {
                    name: name.clone(),
                    n,
                    mean,
                    sd,
                    variance,
                    sem: sd / (n as f64).sqrt(),
                    sum: Some(values.iter().sum()),
                    min: values.iter().cloned().fold(f64::INFINITY, f64::min).into(),
                    max: values
                        .iter()
                        .cloned()
                        .fold(f64::NEG_INFINITY, f64::max)
                        .into(),
                });
                raw.push(values.clone());
            }
            (stats, Some(raw))
        }
        Parsed::Summary(rows) => {
            let stats = rows
                .iter()
                .map(|(name, n, mean, sd)| GroupStats {
                    name: name.clone(),
                    n: *n,
                    mean: *mean,
                    sd: *sd,
                    variance: sd * sd,
                    sem: sd / (*n as f64).sqrt(),
                    sum: None,
                    min: None,
                    max: None,
                })
                .collect();
            notes.push(
                "Input was per-group summary statistics, so Levene's test and the min/max/sum columns are unavailable."
                    .into(),
            );
            (stats, None)
        }
    };

    let k = stats.len();
    if k < 2 {
        return Err(format!(
            "expected at least 2 groups, got {k} — in wide format use one column per group; in long format use \"group,value\" rows (set format=long if auto-detection guessed wrong)"
        ));
    }
    let n_total: usize = stats.iter().map(|g| g.n).sum();
    if n_total <= k {
        return Err(format!(
            "expected more observations than groups: got {n_total} observation(s) across {k} groups, so there are no within-group degrees of freedom"
        ));
    }

    let grand_mean = stats.iter().map(|g| g.n as f64 * g.mean).sum::<f64>() / n_total as f64;
    let (ss_between, ss_within) = match &raw {
        Some(groups) => {
            let (b, w, _) = ss_from_groups(groups);
            (b, w)
        }
        None => {
            let b = stats
                .iter()
                .map(|g| g.n as f64 * (g.mean - grand_mean) * (g.mean - grand_mean))
                .sum::<f64>();
            let w = stats
                .iter()
                .map(|g| (g.n as f64 - 1.0) * g.variance)
                .sum::<f64>();
            (b, w)
        }
    };
    let ss_total = ss_between + ss_within;
    let df_between = k - 1;
    let df_within = n_total - k;
    let df_total = n_total - 1;
    let ms_between = ss_between / df_between as f64;
    let ms_within = ss_within / df_within as f64;

    if ms_within <= 0.0 {
        return Err(
            "within-group variance is zero (every value inside each group is identical), so the F statistic is undefined — add variation or use a different test"
                .into(),
        );
    }

    let f_statistic = ms_between / ms_within;
    let p_value = f_upper_tail(f_statistic, df_between as f64, df_within as f64);
    let f_crit = f_critical(alpha, df_between as f64, df_within as f64);
    let eta_squared = if ss_total > 0.0 {
        ss_between / ss_total
    } else {
        0.0
    };
    let omega_num = ss_between - df_between as f64 * ms_within;
    let omega_squared = omega_num / (ss_total + ms_within);

    // Balanced-design note (affects which post-hoc reading is exact).
    let sizes: Vec<usize> = stats.iter().map(|g| g.n).collect();
    if sizes.iter().any(|n| *n != sizes[0]) {
        notes.push(
            "Group sizes are unequal (unbalanced design); Tukey comparisons use the Tukey–Kramer adjustment."
                .into(),
        );
    }

    // ---- Welch's ANOVA (unequal variances) --------------------------------
    let (mut welch_f, mut welch_df1, mut welch_df2, mut welch_p) = (None, None, None, None);
    if stats.iter().all(|g| g.n >= 2 && g.variance > 0.0) {
        let w: Vec<f64> = stats.iter().map(|g| g.n as f64 / g.variance).collect();
        let sum_w: f64 = w.iter().sum();
        let mean_w = stats.iter().zip(&w).map(|(g, wi)| wi * g.mean).sum::<f64>() / sum_w;
        let kf = k as f64;
        let lambda: f64 = stats
            .iter()
            .zip(&w)
            .map(|(g, wi)| {
                let t = 1.0 - wi / sum_w;
                t * t / (g.n as f64 - 1.0)
            })
            .sum();
        let a = stats
            .iter()
            .zip(&w)
            .map(|(g, wi)| wi * (g.mean - mean_w) * (g.mean - mean_w))
            .sum::<f64>()
            / (kf - 1.0);
        let b = 1.0 + (2.0 * (kf - 2.0) / (kf * kf - 1.0)) * lambda;
        let f = a / b;
        let df2 = (kf * kf - 1.0) / (3.0 * lambda);
        welch_f = Some(f);
        welch_df1 = Some(kf - 1.0);
        welch_df2 = Some(df2);
        welch_p = Some(f_upper_tail(f, kf - 1.0, df2));
    } else {
        notes.push(
            "Welch's ANOVA was skipped: it needs at least 2 observations and non-zero variance in every group."
                .into(),
        );
    }

    // ---- Levene's test (Brown–Forsythe, median-centred) --------------------
    let (mut levene_f, mut levene_df1, mut levene_df2, mut levene_p) = (None, None, None, None);
    if let Some(groups) = &raw {
        let dev: Vec<Vec<f64>> = groups
            .iter()
            .map(|g| {
                let med = median_of(g);
                g.iter().map(|v| (v - med).abs()).collect()
            })
            .collect();
        let (b, w, _) = ss_from_groups(&dev);
        let msw = w / df_within as f64;
        if msw > 0.0 {
            let f = (b / df_between as f64) / msw;
            levene_f = Some(f);
            levene_df1 = Some(df_between);
            levene_df2 = Some(df_within);
            levene_p = Some(f_upper_tail(f, df_between as f64, df_within as f64));
        } else {
            notes.push(
                "Levene's test was skipped: the absolute deviations from each group median have zero within-group spread."
                    .into(),
            );
        }
    }

    // ---- post-hoc pairwise comparisons ------------------------------------
    let mut comparisons: Vec<PairComparison> = Vec::new();
    if method == "games-howell" && !stats.iter().all(|g| g.n >= 2 && g.variance > 0.0) {
        return Err(
            "the Games-Howell post-hoc test needs at least 2 observations and non-zero variance in every group — use posthoc=tukey (or bonferroni/holm) for this data"
                .into(),
        );
    }
    if method != "none" {
        let dfw = df_within as f64;
        let m = (k * (k - 1) / 2) as f64;
        let q_crit = if method == "tukey" {
            tukey_critical(alpha, k as f64, dfw)
        } else {
            0.0
        };
        let t_crit_lsd = t_critical(alpha, dfw);
        let t_crit_bonf = t_critical(alpha / m, dfw);
        for i in 0..k {
            for j in (i + 1)..k {
                let a = &stats[i];
                let b = &stats[j];
                let diff = a.mean - b.mean;
                let inv = 1.0 / a.n as f64 + 1.0 / b.n as f64;
                let se_t = (ms_within * inv).sqrt();
                let se_q = (ms_within * inv / 2.0).sqrt();
                // Per-pair standard error actually used by the chosen method
                // (Tukey's q-scale SE, Games-Howell's Welch SE, or the pooled t SE).
                let mut se_used = if method == "tukey" { se_q } else { se_t };
                let (statistic, p_raw, p_adj, ci) = match method.as_str() {
                    "tukey" => {
                        let q = (diff / se_q).abs();
                        let p = tukey_upper_tail(q, k as f64, dfw);
                        let half = q_crit * se_q;
                        (q, p, p, Some((diff - half, diff + half)))
                    }
                    // Games-Howell: Tukey's studentized range on each pair's own
                    // variances with Welch-Satterthwaite degrees of freedom, so
                    // it stays valid when the groups have unequal variances.
                    "games-howell" => {
                        let va = a.variance / a.n as f64;
                        let vb = b.variance / b.n as f64;
                        let se_gh = (0.5 * (va + vb)).sqrt();
                        se_used = se_gh;
                        let df_gh = (va + vb) * (va + vb)
                            / (va * va / (a.n as f64 - 1.0) + vb * vb / (b.n as f64 - 1.0));
                        let q = (diff / se_gh).abs();
                        let p = tukey_upper_tail(q, k as f64, df_gh);
                        let half = tukey_critical(alpha, k as f64, df_gh) * se_gh;
                        (q, p, p, Some((diff - half, diff + half)))
                    }
                    "lsd" => {
                        let t = diff / se_t;
                        let p = student_t_two_tail(t, dfw);
                        let half = t_crit_lsd * se_t;
                        (t, p, p, Some((diff - half, diff + half)))
                    }
                    "bonferroni" => {
                        let t = diff / se_t;
                        let p = student_t_two_tail(t, dfw);
                        let half = t_crit_bonf * se_t;
                        (t, p, (p * m).min(1.0), Some((diff - half, diff + half)))
                    }
                    // holm: adjusted below, once every raw p is known
                    _ => {
                        let t = diff / se_t;
                        let p = student_t_two_tail(t, dfw);
                        (t, p, p, None)
                    }
                };
                comparisons.push(PairComparison {
                    group_a: a.name.clone(),
                    group_b: b.name.clone(),
                    mean_difference: diff,
                    std_error: se_used,
                    statistic,
                    p_value: p_raw,
                    p_adjusted: p_adj,
                    ci_lower: ci.map(|c| c.0),
                    ci_upper: ci.map(|c| c.1),
                    significant: p_adj < alpha,
                });
            }
        }
        if method == "holm" {
            // Holm step-down: sort ascending, adjusted_i = max over prefix of (m − i) · p_i.
            let mut idx: Vec<usize> = (0..comparisons.len()).collect();
            idx.sort_by(|&x, &y| {
                comparisons[x]
                    .p_value
                    .partial_cmp(&comparisons[y].p_value)
                    .unwrap()
            });
            let mut running = 0.0f64;
            for (rank, &i) in idx.iter().enumerate() {
                let adj = ((m - rank as f64) * comparisons[i].p_value).min(1.0);
                running = running.max(adj);
                comparisons[i].p_adjusted = running;
                comparisons[i].significant = running < alpha;
            }
        }
    }

    Ok(AnovaResult {
        test: "one-way ANOVA".into(),
        input_format,
        groups: stats,
        group_count: k,
        observations: n_total,
        grand_mean,
        ss_between,
        df_between,
        ms_between,
        ss_within,
        df_within,
        ms_within,
        ss_total,
        df_total,
        f_statistic,
        p_value,
        f_critical: f_crit,
        alpha,
        reject_null: p_value < alpha,
        eta_squared,
        omega_squared,
        welch_f,
        welch_df1,
        welch_df2,
        welch_p_value: welch_p,
        levene_f,
        levene_df1,
        levene_df2,
        levene_p_value: levene_p,
        posthoc: method,
        comparisons,
        notes,
    })
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

fn fmt_num(v: f64, d: usize) -> String {
    if v == 0.0 {
        // avoid "-0.0000"
        return format!("{:.*}", d, 0.0);
    }
    format!("{:.*}", d, v)
}

/// p-values below the display resolution print as "< 0.0001" rather than "0.0000".
fn fmt_p(p: f64, d: usize) -> String {
    let eps = 10f64.powi(-(d as i32));
    if p < eps {
        format!("< {}", fmt_num(eps, d))
    } else {
        fmt_num(p, d)
    }
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

fn effect_label(eta: f64) -> &'static str {
    if eta < 0.01 {
        "negligible"
    } else if eta < 0.06 {
        "small"
    } else if eta < 0.14 {
        "medium"
    } else {
        "large"
    }
}

/// Plain-text report (the page's default view).
pub fn render_summary(r: &AnovaResult, d: usize) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "One-way ANOVA\ngroups: {}\nobservations: {}\ngrand mean: {}\n\n",
        r.group_count,
        r.observations,
        fmt_num(r.grand_mean, d)
    ));

    // Group descriptive statistics.
    let name_w = r
        .groups
        .iter()
        .map(|g| g.name.chars().count())
        .max()
        .unwrap_or(5)
        .max(5);
    out.push_str("Group statistics\n");
    out.push_str(&format!(
        "{}  {}  {}  {}  {}\n",
        pad_right("group", name_w),
        pad_left("n", 5),
        pad_left("mean", 12),
        pad_left("sd", 12),
        pad_left("sem", 12)
    ));
    for g in &r.groups {
        out.push_str(&format!(
            "{}  {}  {}  {}  {}\n",
            pad_right(&g.name, name_w),
            pad_left(&g.n.to_string(), 5),
            pad_left(&fmt_num(g.mean, d), 12),
            pad_left(&fmt_num(g.sd, d), 12),
            pad_left(&fmt_num(g.sem, d), 12)
        ));
    }

    // ANOVA table.
    out.push_str("\nANOVA table\n");
    out.push_str(&format!(
        "{}  {}  {}  {}  {}  {}\n",
        pad_right("source", 16),
        pad_left("SS", 14),
        pad_left("df", 6),
        pad_left("MS", 14),
        pad_left("F", 12),
        pad_left("p", 12)
    ));
    out.push_str(&format!(
        "{}  {}  {}  {}  {}  {}\n",
        pad_right("between groups", 16),
        pad_left(&fmt_num(r.ss_between, d), 14),
        pad_left(&r.df_between.to_string(), 6),
        pad_left(&fmt_num(r.ms_between, d), 14),
        pad_left(&fmt_num(r.f_statistic, d), 12),
        pad_left(&fmt_p(r.p_value, d), 12)
    ));
    out.push_str(&format!(
        "{}  {}  {}  {}  {}  {}\n",
        pad_right("within groups", 16),
        pad_left(&fmt_num(r.ss_within, d), 14),
        pad_left(&r.df_within.to_string(), 6),
        pad_left(&fmt_num(r.ms_within, d), 14),
        pad_left("", 12),
        pad_left("", 12)
    ));
    out.push_str(&format!(
        "{}  {}  {}  {}  {}  {}\n",
        pad_right("total", 16),
        pad_left(&fmt_num(r.ss_total, d), 14),
        pad_left(&r.df_total.to_string(), 6),
        pad_left("", 14),
        pad_left("", 12),
        pad_left("", 12)
    ));

    out.push_str(&format!(
        "\nF({}, {}) = {}, p = {}\ncritical F at alpha {} = {}\n",
        r.df_between,
        r.df_within,
        fmt_num(r.f_statistic, d),
        fmt_p(r.p_value, d),
        fmt_num(r.alpha, 4),
        fmt_num(r.f_critical, d)
    ));
    out.push_str(&if r.reject_null {
        format!(
            "result: p < alpha {} -> reject the null hypothesis; at least one group mean differs\n",
            fmt_num(r.alpha, 4)
        )
    } else {
        format!(
            "result: p >= alpha {} -> fail to reject the null hypothesis; no significant difference between group means\n",
            fmt_num(r.alpha, 4)
        )
    });
    out.push_str(&format!(
        "effect size: eta-squared = {} ({}), omega-squared = {}\n",
        fmt_num(r.eta_squared, d),
        effect_label(r.eta_squared),
        fmt_num(r.omega_squared, d)
    ));

    // Assumption checks.
    if r.levene_f.is_some() || r.welch_f.is_some() {
        out.push_str("\nAssumption checks\n");
    }
    if let (Some(f), Some(d1), Some(d2), Some(p)) =
        (r.levene_f, r.levene_df1, r.levene_df2, r.levene_p_value)
    {
        let verdict = if p < r.alpha {
            "unequal variances indicated - prefer Welch's ANOVA"
        } else {
            "equal variances are plausible"
        };
        out.push_str(&format!(
            "Levene (Brown-Forsythe): F({d1}, {d2}) = {}, p = {} -> {verdict}\n",
            fmt_num(f, d),
            fmt_p(p, d)
        ));
    }
    if let (Some(f), Some(d1), Some(d2), Some(p)) =
        (r.welch_f, r.welch_df1, r.welch_df2, r.welch_p_value)
    {
        out.push_str(&format!(
            "Welch's ANOVA: F({}, {}) = {}, p = {}\n",
            fmt_num(d1, 0),
            fmt_num(d2, 2),
            fmt_num(f, d),
            fmt_p(p, d)
        ));
    }

    if !r.comparisons.is_empty() {
        let label = match r.posthoc.as_str() {
            "tukey" => "Post-hoc: Tukey HSD (q statistic)",
            "games-howell" => "Post-hoc: Games-Howell (q statistic, Welch degrees of freedom)",
            "lsd" => "Post-hoc: Fisher's LSD (unadjusted t)",
            "bonferroni" => "Post-hoc: Bonferroni-adjusted t",
            _ => "Post-hoc: Holm-adjusted t",
        };
        out.push_str(&format!("\n{label}\n"));
        let pair_w = r
            .comparisons
            .iter()
            .map(|c| c.group_a.chars().count() + c.group_b.chars().count() + 5)
            .max()
            .unwrap_or(12)
            .max(12);
        let stat_head = if r.posthoc == "tukey" { "q" } else { "t" };
        out.push_str(&format!(
            "{}  {}  {}  {}  {}\n",
            pad_right("comparison", pair_w),
            pad_left("diff", 12),
            pad_left(stat_head, 10),
            pad_left("p adj", 10),
            pad_right("", 4)
        ));
        for c in &r.comparisons {
            let pair = format!("{} vs {}", c.group_a, c.group_b);
            out.push_str(&format!(
                "{}  {}  {}  {}  {}\n",
                pad_right(&pair, pair_w),
                pad_left(&fmt_num(c.mean_difference, d), 12),
                pad_left(&fmt_num(c.statistic, d), 10),
                pad_left(&fmt_p(c.p_adjusted, d), 10),
                if c.significant { "*" } else { "" }
            ));
        }
        out.push_str(&format!("* significant at alpha {}\n", fmt_num(r.alpha, 4)));
    }

    for n in &r.notes {
        out.push_str(&format!("\nnote: {n}"));
    }
    if !r.notes.is_empty() {
        out.push('\n');
    }
    out
}

/// Markdown tables — paste straight into a report or issue.
pub fn render_table(r: &AnovaResult, d: usize) -> String {
    let mut out =
        String::from("| group | n | mean | sd | sem |\n| --- | --- | --- | --- | --- |\n");
    for g in &r.groups {
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} |\n",
            g.name,
            g.n,
            fmt_num(g.mean, d),
            fmt_num(g.sd, d),
            fmt_num(g.sem, d)
        ));
    }
    out.push_str("\n| source | SS | df | MS | F | p |\n| --- | --- | --- | --- | --- | --- |\n");
    out.push_str(&format!(
        "| between groups | {} | {} | {} | {} | {} |\n",
        fmt_num(r.ss_between, d),
        r.df_between,
        fmt_num(r.ms_between, d),
        fmt_num(r.f_statistic, d),
        fmt_p(r.p_value, d)
    ));
    out.push_str(&format!(
        "| within groups | {} | {} | {} |  |  |\n",
        fmt_num(r.ss_within, d),
        r.df_within,
        fmt_num(r.ms_within, d)
    ));
    out.push_str(&format!(
        "| total | {} | {} |  |  |  |\n",
        fmt_num(r.ss_total, d),
        r.df_total
    ));
    if !r.comparisons.is_empty() {
        let stat_head = if r.posthoc == "tukey" { "q" } else { "t" };
        out.push_str(&format!(
            "\n| comparison | diff | {stat_head} | p adj | significant |\n| --- | --- | --- | --- | --- |\n"
        ));
        for c in &r.comparisons {
            out.push_str(&format!(
                "| {} vs {} | {} | {} | {} | {} |\n",
                c.group_a,
                c.group_b,
                fmt_num(c.mean_difference, d),
                fmt_num(c.statistic, d),
                fmt_p(c.p_adjusted, d),
                if c.significant { "yes" } else { "no" }
            ));
        }
    }
    out
}

/// Round every reported number to `decimals` places for the JSON view.
fn rounded(r: &AnovaResult, d: usize) -> AnovaResult {
    // p-values keep at least 6 places so tiny ones don't collapse to 0.
    let pd = d.max(6);
    let mut c = r.clone();
    c.grand_mean = round_to(c.grand_mean, d);
    c.ss_between = round_to(c.ss_between, d);
    c.ms_between = round_to(c.ms_between, d);
    c.ss_within = round_to(c.ss_within, d);
    c.ms_within = round_to(c.ms_within, d);
    c.ss_total = round_to(c.ss_total, d);
    c.f_statistic = round_to(c.f_statistic, d);
    c.p_value = round_to(c.p_value, pd);
    c.f_critical = round_to(c.f_critical, d);
    c.eta_squared = round_to(c.eta_squared, d);
    c.omega_squared = round_to(c.omega_squared, d);
    c.welch_f = c.welch_f.map(|v| round_to(v, d));
    c.welch_df2 = c.welch_df2.map(|v| round_to(v, d));
    c.welch_p_value = c.welch_p_value.map(|v| round_to(v, pd));
    c.levene_f = c.levene_f.map(|v| round_to(v, d));
    c.levene_p_value = c.levene_p_value.map(|v| round_to(v, pd));
    for g in &mut c.groups {
        g.mean = round_to(g.mean, d);
        g.sd = round_to(g.sd, d);
        g.variance = round_to(g.variance, d);
        g.sem = round_to(g.sem, d);
        g.sum = g.sum.map(|v| round_to(v, d));
        g.min = g.min.map(|v| round_to(v, d));
        g.max = g.max.map(|v| round_to(v, d));
    }
    for p in &mut c.comparisons {
        p.mean_difference = round_to(p.mean_difference, d);
        p.std_error = round_to(p.std_error, d);
        p.statistic = round_to(p.statistic, d);
        p.p_value = round_to(p.p_value, pd);
        p.p_adjusted = round_to(p.p_adjusted, pd);
        p.ci_lower = p.ci_lower.map(|v| round_to(v, d));
        p.ci_upper = p.ci_upper.map(|v| round_to(v, d));
    }
    c
}

/// The single entry point shared by the chat block, the CLI and the page.
#[allow(clippy::too_many_arguments)]
pub fn run(
    data: &str,
    format: &str,
    delimiter: &str,
    header: &str,
    alpha: f64,
    decimals: f64,
    posthoc: &str,
    output: &str,
) -> Result<String, String> {
    if !decimals.is_finite() || decimals.fract() != 0.0 || !(0.0..=10.0).contains(&decimals) {
        return Err(format!(
            "invalid decimals {decimals}: expected a whole number between 0 and 10"
        ));
    }
    let d = decimals as usize;
    let mode = output.trim().to_ascii_lowercase();
    let r = analyze(data, format, delimiter, header, alpha, posthoc)?;
    match mode.as_str() {
        "" | "summary" => Ok(render_summary(&r, d)),
        "table" => Ok(render_table(&r, d)),
        "json" => serde_json::to_string_pretty(&rounded(&r, d))
            .map_err(|e| format!("failed to serialize the result as JSON: {e}")),
        other => Err(format!(
            "invalid output {other:?}: expected \"summary\", \"table\" or \"json\""
        )),
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

    #[test]
    fn f_upper_tail_matches_r_pf_values() {
        // pf(4.4573, 2, 12, lower.tail = FALSE) = 0.0356771...
        // For df1 = 2 the upper tail is exactly (1 + 2f/df2)^(-df2/2).
        assert!(close(f_upper_tail(4.4573, 2.0, 12.0), 0.035_677_145, 1e-8));
        // pf(1.0, 5, 5, lower.tail = FALSE) = 0.5
        assert!(close(f_upper_tail(1.0, 5.0, 5.0), 0.5, 1e-9));
        // pf(2.0, 1, 1, lower.tail = FALSE) = 0.39182655
        assert!(close(f_upper_tail(2.0, 1.0, 1.0), 0.391_826_552, 1e-8));
        assert!(close(f_upper_tail(0.0, 3.0, 9.0), 1.0, 1e-12));
    }

    #[test]
    fn f_critical_inverts_the_upper_tail() {
        // qf(0.95, 2, 12) = 3.885294
        assert!(close(f_critical(0.05, 2.0, 12.0), 3.885_294, 1e-4));
        // qf(0.99, 3, 20) = 4.938193
        assert!(close(f_critical(0.01, 3.0, 20.0), 4.938_193, 1e-4));
        let c = f_critical(0.05, 4.0, 30.0);
        assert!(close(f_upper_tail(c, 4.0, 30.0), 0.05, 1e-9));
    }

    #[test]
    fn student_t_two_tail_matches_known_values() {
        // 2 * pt(-2.228, 10) = 0.05001...
        assert!(close(student_t_two_tail(2.228, 10.0), 0.050_005, 1e-5));
        assert!(close(student_t_two_tail(0.0, 10.0), 1.0, 1e-12));
    }

    #[test]
    fn tukey_with_two_groups_equals_the_two_tailed_t() {
        // For k = 2 the studentized range is exactly |t|·√2, so the tail
        // probabilities must agree — the strongest available cross-check.
        for &(t, df) in &[(1.0, 5.0), (2.228, 10.0), (0.4, 30.0), (3.5, 12.0)] {
            let q = t * std::f64::consts::SQRT_2;
            let expected = student_t_two_tail(t, df);
            let got = tukey_upper_tail(q, 2.0, df);
            assert!(
                close(got, expected, 1e-6),
                "k=2 tukey {got} vs t {expected} (t={t}, df={df})"
            );
        }
    }

    #[test]
    fn tukey_critical_matches_published_tables() {
        // Standard studentized-range tables: q(0.05; k, df).
        assert!(close(tukey_critical(0.05, 3.0, 12.0), 3.773, 2e-3));
        assert!(close(tukey_critical(0.05, 4.0, 20.0), 3.958, 2e-3));
        assert!(close(tukey_critical(0.05, 5.0, 10.0), 4.654, 3e-3));
        assert!(close(tukey_critical(0.01, 3.0, 12.0), 5.046, 4e-3));
    }

    // ---- ANOVA ------------------------------------------------------------

    /// Worked example: three groups of five observations, one column per group.
    /// Group 1 = 5,6,9,9,11 (mean 8.0), Group 2 = 5,7,9,10,11 (mean 8.4),
    /// Group 3 = 8,11,13,13,14 (mean 11.8). Chosen so the sums of squares are
    /// exact in binary: SS_between = 43.6, SS_within = 70, F(2, 12) = 3.7371,
    /// p = 0.0547 — a deliberately borderline result just above alpha = 0.05.
    const WIDE: &str = "5,5,8\n6,7,11\n9,9,13\n9,10,13\n11,11,14";

    #[test]
    fn wide_input_produces_the_textbook_anova_table() {
        let r = analyze(WIDE, "auto", "auto", "auto", 0.05, "none").unwrap();
        assert_eq!(r.input_format, "wide");
        assert_eq!(r.group_count, 3);
        assert_eq!(r.observations, 15);
        assert!(close(r.grand_mean, 9.4, 1e-12));
        assert!(close(r.ss_between, 43.6, 1e-9));
        assert!(close(r.ss_within, 70.0, 1e-9));
        assert!(close(r.ss_total, 113.6, 1e-9));
        assert_eq!((r.df_between, r.df_within, r.df_total), (2, 12, 14));
        assert!(close(r.ms_between, 21.8, 1e-9));
        assert!(close(r.ms_within, 5.833_333_333, 1e-8));
        assert!(close(r.f_statistic, 3.737_142_857, 1e-8));
        assert!(close(r.p_value, 0.054_749, 1e-5));
        assert!(!r.reject_null);
        assert!(close(r.eta_squared, 0.383_802_816, 1e-8));
        assert_eq!(r.groups[0].name, "Group 1");
        assert!(close(r.groups[0].mean, 8.0, 1e-12));
        assert!(close(r.groups[2].mean, 11.8, 1e-12));
    }

    #[test]
    fn long_input_matches_the_same_wide_data() {
        let long =
            "a,5\na,6\na,9\na,9\na,11\nb,5\nb,7\nb,9\nb,10\nb,11\nc,8\nc,11\nc,13\nc,13\nc,14";
        let r = analyze(long, "auto", "auto", "auto", 0.05, "none").unwrap();
        assert_eq!(r.input_format, "long");
        assert_eq!(r.group_count, 3);
        assert_eq!(r.groups[0].name, "a");
        assert!(close(r.f_statistic, 3.737_142_857, 1e-8));
        assert!(close(r.ss_between, 43.6, 1e-9));
    }

    #[test]
    fn long_input_with_a_header_row_is_detected() {
        let long = "group,value\nx,1\nx,2\nx,3\ny,5\ny,6\ny,7";
        let r = analyze(long, "auto", "auto", "auto", 0.05, "none").unwrap();
        assert_eq!(r.groups.len(), 2);
        assert_eq!(r.groups[0].name, "x");
        assert_eq!(r.observations, 6);
        // Means 2 and 6: SS_between = 24, SS_within = 4 on 4 df, so F = 24.
        assert!(close(r.f_statistic, 24.0, 1e-9));
    }

    #[test]
    fn wide_input_with_named_header_columns() {
        let wide = "control\ttreatment\n1\t5\n2\t6\n3\t7";
        let r = analyze(wide, "wide", "tab", "yes", 0.05, "none").unwrap();
        assert_eq!(r.groups[0].name, "control");
        assert_eq!(r.groups[1].name, "treatment");
        assert!(close(r.f_statistic, 24.0, 1e-9));
    }

    #[test]
    fn ragged_wide_columns_allow_unequal_group_sizes() {
        let wide = "1,10\n2,11\n3,12\n4,";
        let r = analyze(wide, "wide", "comma", "no", 0.05, "none").unwrap();
        assert_eq!(r.groups[0].n, 4);
        assert_eq!(r.groups[1].n, 3);
        assert!(r.notes.iter().any(|n| n.contains("unequal")));
    }

    #[test]
    fn summary_statistics_input_reproduces_the_raw_result() {
        // Same data as WIDE, expressed as n / mean / sd per group.
        let raw = analyze(WIDE, "wide", "comma", "no", 0.05, "none").unwrap();
        let summary = format!(
            "name,n,mean,sd\nGroup 1,5,{},{}\nGroup 2,5,{},{}\nGroup 3,5,{},{}",
            raw.groups[0].mean,
            raw.groups[0].sd,
            raw.groups[1].mean,
            raw.groups[1].sd,
            raw.groups[2].mean,
            raw.groups[2].sd
        );
        let r = analyze(&summary, "summary", "comma", "auto", 0.05, "none").unwrap();
        assert_eq!(r.input_format, "summary");
        assert!(close(r.f_statistic, raw.f_statistic, 1e-9));
        assert!(close(r.p_value, raw.p_value, 1e-12));
        assert!(r.levene_f.is_none());
        assert!(r.groups[0].min.is_none());
    }

    #[test]
    fn welch_and_levene_are_reported_for_raw_data() {
        let r = analyze(WIDE, "wide", "comma", "no", 0.05, "none").unwrap();
        // oneway.test(var.equal = FALSE) => F = 3.48263, num df = 2,
        // denom df = 7.99910, p = 0.08167 (group variances 6.0, 5.8, 5.7).
        assert!(close(r.welch_f.unwrap(), 3.482_627, 1e-5));
        assert!(close(r.welch_df2.unwrap(), 7.999_100, 1e-5));
        assert!(close(r.welch_p_value.unwrap(), 0.081_669, 1e-5));
        // Brown-Forsythe Levene on the same data.
        assert!(r.levene_f.is_some());
        assert_eq!(r.levene_df1, Some(2));
        assert_eq!(r.levene_df2, Some(12));
        assert!(r.levene_p_value.unwrap() > 0.05);
    }

    #[test]
    fn tukey_posthoc_lists_every_pair() {
        let r = analyze(WIDE, "wide", "comma", "auto", 0.05, "tukey").unwrap();
        assert_eq!(r.posthoc, "tukey");
        assert_eq!(r.comparisons.len(), 3);
        let c = &r.comparisons[1]; // Group 1 vs Group 3
        assert_eq!(
            (c.group_a.as_str(), c.group_b.as_str()),
            ("Group 1", "Group 3")
        );
        assert!(close(c.mean_difference, -3.8, 1e-9));
        // TukeyHSD: q = 3.5181 on k = 3, df = 12 => p adj = 0.06846 for the
        // 8.0 vs 11.8 pair, so the widest gap still misses alpha = 0.05.
        assert!(close(c.p_adjusted, 0.068_463, 5e-4), "got {}", c.p_adjusted);
        assert!(!c.significant);
        assert!(c.ci_lower.unwrap() < c.mean_difference);
    }

    #[test]
    fn bonferroni_and_holm_adjust_the_same_raw_p_values() {
        let b = analyze(WIDE, "wide", "comma", "auto", 0.05, "bonferroni").unwrap();
        let h = analyze(WIDE, "wide", "comma", "auto", 0.05, "holm").unwrap();
        assert_eq!(b.comparisons.len(), 3);
        for (x, y) in b.comparisons.iter().zip(&h.comparisons) {
            assert!(close(x.p_value, y.p_value, 1e-12));
            // Holm is never more conservative than Bonferroni.
            assert!(y.p_adjusted <= x.p_adjusted + 1e-12);
        }
        // Raw p for Group 1 vs Group 3: t = -3.8/sqrt(5.8333*0.4) = -2.48768,
        // p = 0.028553 on 12 df; Bonferroni multiplies by the 3 pairs.
        assert!(close(b.comparisons[1].p_value, 0.028_553, 1e-6));
        assert!(close(b.comparisons[1].p_adjusted, 0.085_659, 1e-6));
        assert!(h.comparisons[1].ci_lower.is_none());
    }

    #[test]
    fn lsd_posthoc_is_unadjusted() {
        let r = analyze(WIDE, "wide", "comma", "auto", 0.05, "lsd").unwrap();
        assert!(close(
            r.comparisons[1].p_value,
            r.comparisons[1].p_adjusted,
            1e-12
        ));
    }

    #[test]
    fn alpha_changes_the_critical_value_and_the_verdict() {
        let a = analyze(WIDE, "wide", "comma", "auto", 0.05, "none").unwrap();
        let b = analyze(WIDE, "wide", "comma", "auto", 0.10, "none").unwrap();
        assert!(!a.reject_null);
        assert!(b.reject_null);
        assert!(b.f_critical < a.f_critical);
    }

    #[test]
    fn semicolon_and_pipe_delimiters_parse() {
        let semi = analyze("1;10\n2;11\n3;12", "wide", "semicolon", "no", 0.05, "none").unwrap();
        let pipe = analyze("1|10\n2|11\n3|12", "wide", "pipe", "no", 0.05, "none").unwrap();
        assert!(close(semi.f_statistic, pipe.f_statistic, 1e-12));
        assert_eq!(semi.group_count, 2);
    }

    #[test]
    fn comment_and_blank_lines_are_ignored() {
        let r = analyze(
            "# my data\n\n1,10\n2,11\n\n3,12\n",
            "wide",
            "comma",
            "no",
            0.05,
            "none",
        )
        .unwrap();
        assert_eq!(r.observations, 6);
    }

    // ---- errors -----------------------------------------------------------

    #[test]
    fn empty_input_is_rejected() {
        let e = analyze("   \n\n", "auto", "auto", "auto", 0.05, "none").unwrap_err();
        assert!(e.contains("no data"), "{e}");
    }

    #[test]
    fn a_single_group_is_rejected_with_a_hint() {
        let e = analyze("1\n2\n3\n4", "auto", "auto", "auto", 0.05, "none").unwrap_err();
        assert!(e.contains("at least 2 groups"), "{e}");
    }

    #[test]
    fn non_numeric_cells_report_the_line() {
        let e = analyze("1,2\n3,oops", "wide", "comma", "no", 0.05, "none").unwrap_err();
        assert!(e.contains("line 2") && e.contains("oops"), "{e}");
    }

    #[test]
    fn zero_within_group_variance_is_rejected() {
        let e = analyze("1,5\n1,5\n1,5", "wide", "comma", "no", 0.05, "none").unwrap_err();
        assert!(e.contains("within-group variance is zero"), "{e}");
    }

    #[test]
    fn out_of_range_alpha_is_rejected() {
        let e = analyze(WIDE, "wide", "comma", "no", 0.9, "none").unwrap_err();
        assert!(e.contains("invalid alpha"), "{e}");
    }

    #[test]
    fn unknown_options_are_rejected_with_the_valid_set() {
        assert!(analyze(WIDE, "wide", "comma", "no", 0.05, "scheffe")
            .unwrap_err()
            .contains("invalid posthoc"));
        assert!(analyze(WIDE, "nope", "comma", "no", 0.05, "none")
            .unwrap_err()
            .contains("invalid format"));
        assert!(analyze(WIDE, "wide", "colon", "no", 0.05, "none")
            .unwrap_err()
            .contains("invalid delimiter"));
        assert!(analyze(WIDE, "wide", "comma", "maybe", 0.05, "none")
            .unwrap_err()
            .contains("invalid header"));
    }

    #[test]
    fn too_few_observations_is_rejected() {
        let e = analyze("1,2", "wide", "comma", "no", 0.05, "none").unwrap_err();
        assert!(e.contains("no within-group degrees of freedom"), "{e}");
    }

    // ---- rendering --------------------------------------------------------

    #[test]
    fn summary_output_contains_the_key_lines() {
        let out = run(WIDE, "auto", "auto", "auto", 0.05, 4.0, "none", "summary").unwrap();
        assert!(out.contains("One-way ANOVA"));
        assert!(out.contains("F(2, 12) = 3.7371, p = 0.0547"));
        assert!(out.contains("critical F at alpha 0.0500 = 3.8853"));
        assert!(out.contains("fail to reject the null hypothesis"));
        assert!(out.contains("eta-squared = 0.3838 (large)"));
        assert!(out.contains("between groups"));
    }

    #[test]
    fn table_output_is_markdown() {
        let out = run(WIDE, "auto", "auto", "auto", 0.05, 2.0, "tukey", "table").unwrap();
        assert!(out.starts_with("| group | n | mean | sd | sem |"));
        assert!(out.contains("| between groups | 43.60 | 2 | 21.80 | 3.74 | 0.05 |"));
        assert!(out.contains("| Group 1 vs Group 3 |"));
    }

    #[test]
    fn json_output_parses_and_rounds() {
        let out = run(WIDE, "auto", "auto", "auto", 0.05, 3.0, "none", "json").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["test"], "one-way ANOVA");
        assert_eq!(v["group_count"], 3);
        assert_eq!(v["f_statistic"], 3.737);
        assert_eq!(v["df_within"], 12);
        assert_eq!(v["groups"][0]["mean"], 8.0);
        assert_eq!(v["groups"][0]["max"], 11.0);
    }

    #[test]
    fn decimals_control_the_text_precision() {
        let out = run(WIDE, "auto", "auto", "auto", 0.05, 1.0, "none", "summary").unwrap();
        assert!(out.contains("F(2, 12) = 3.7, p = < 0.1"), "{out}");
    }

    #[test]
    fn tiny_p_values_print_below_the_resolution() {
        let wide = "1,100\n2,101\n3,102\n4,103\n5,104";
        let out = run(wide, "wide", "comma", "no", 0.05, 4.0, "none", "summary").unwrap();
        assert!(out.contains("p = < 0.0001"), "{out}");
    }

    #[test]
    fn invalid_output_and_decimals_are_rejected() {
        assert!(run(WIDE, "auto", "auto", "auto", 0.05, 4.0, "none", "xml")
            .unwrap_err()
            .contains("invalid output"));
        assert!(
            run(WIDE, "auto", "auto", "auto", 0.05, 42.0, "none", "summary")
                .unwrap_err()
                .contains("invalid decimals")
        );
    }
}
