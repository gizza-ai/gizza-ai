//! gizza-ai/chi-square-test core — Pearson's chi-square test.
//!
//! Two modes:
//!   * **goodness-of-fit** — compare an observed frequency vector against an
//!     expected one (equal frequencies by default, or explicit expected counts /
//!     ratios). df = k - 1.
//!   * **contingency / independence** — an r×c table of observed counts; tests
//!     whether the row and column variables are independent. Expected counts are
//!     the row-total × column-total / grand-total. df = (r-1)(c-1).
//!
//! The statistic is χ² = Σ (O − E)² / E. The p-value is the upper tail of the
//! χ² distribution with the given degrees of freedom, computed from the
//! regularized upper incomplete gamma function (pure Rust, no deps besides serde).

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ChiSquareResult {
    /// "goodness-of-fit" or "contingency".
    pub test: String,
    pub chi_square: f64,
    pub degrees_of_freedom: usize,
    pub p_value: f64,
    /// Grand total of observed counts.
    pub total_observed: f64,
    /// Per-cell expected counts (flattened row-major for contingency tables).
    pub expected: Vec<f64>,
    /// Number of cells with expected count < 5 (assumption-warning aid).
    pub cells_expected_below_5: usize,
    /// Cramér's V effect size (contingency only); null for goodness-of-fit.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cramers_v: Option<f64>,
    /// True when one or more expected cells are < 5 (χ² approximation is weak).
    pub low_expected_warning: bool,
    /// True when Yates' continuity correction was applied (2×2 contingency only).
    pub yates_correction: bool,
}

fn round6(v: f64) -> f64 {
    (v * 1e6).round() / 1e6
}

// ---- incomplete gamma / chi-square p-value -------------------------------

/// ln(Γ(x)) — Lanczos approximation.
fn ln_gamma(x: f64) -> f64 {
    // Lanczos g=7, n=9 coefficients.
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
        // reflection: Γ(x)Γ(1-x) = π / sin(πx)
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

/// Regularized lower incomplete gamma P(s, x) via series expansion.
fn gamma_p_series(s: f64, x: f64) -> f64 {
    let mut sum = 1.0 / s;
    let mut term = sum;
    let mut n = 1.0;
    loop {
        term *= x / (s + n);
        sum += term;
        if term.abs() < sum.abs() * 1e-15 || n > 10_000.0 {
            break;
        }
        n += 1.0;
    }
    sum * (-x + s * x.ln() - ln_gamma(s)).exp()
}

/// Regularized upper incomplete gamma Q(s, x) via continued fraction (Lentz).
fn gamma_q_cf(s: f64, x: f64) -> f64 {
    let tiny = 1e-300;
    let mut b = x + 1.0 - s;
    let mut c = 1.0 / tiny;
    let mut d = 1.0 / b;
    let mut h = d;
    let mut i = 1.0;
    loop {
        let an = -i * (i - s);
        b += 2.0;
        d = an * d + b;
        if d.abs() < tiny {
            d = tiny;
        }
        c = b + an / c;
        if c.abs() < tiny {
            c = tiny;
        }
        d = 1.0 / d;
        let del = d * c;
        h *= del;
        if (del - 1.0).abs() < 1e-15 || i > 10_000.0 {
            break;
        }
        i += 1.0;
    }
    (-x + s * x.ln() - ln_gamma(s)).exp() * h
}

/// Regularized upper incomplete gamma Q(s, x) = 1 − P(s, x).
fn gamma_q(s: f64, x: f64) -> f64 {
    if x <= 0.0 {
        return 1.0;
    }
    if x < s + 1.0 {
        1.0 - gamma_p_series(s, x)
    } else {
        gamma_q_cf(s, x)
    }
}

/// Upper-tail p-value of the chi-square distribution: P(X² ≥ stat | df).
fn chi_square_p_value(stat: f64, df: usize) -> f64 {
    if df == 0 {
        return f64::NAN;
    }
    if stat <= 0.0 {
        return 1.0;
    }
    gamma_q(df as f64 / 2.0, stat / 2.0).clamp(0.0, 1.0)
}

// ---- parsing -------------------------------------------------------------

/// Parse a single row of whitespace/comma/semicolon-separated non-negative numbers.
fn parse_row(text: &str) -> Result<Vec<f64>, String> {
    let mut nums = Vec::new();
    for tok in text.split(|c: char| c.is_whitespace() || c == ',' || c == ';') {
        let t = tok.trim();
        if t.is_empty() {
            continue;
        }
        let n: f64 = t.parse().map_err(|_| format!("'{t}' is not a number"))?;
        if !n.is_finite() {
            return Err(format!("'{t}' is not a finite number"));
        }
        nums.push(n);
    }
    Ok(nums)
}

/// Parse a table: each non-empty line is a row, cells separated by spaces /
/// commas / semicolons / tabs. Returns the rows (validated to be equal width).
fn parse_table(text: &str) -> Result<Vec<Vec<f64>>, String> {
    let mut rows: Vec<Vec<f64>> = Vec::new();
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let row = parse_row(line)?;
        if !row.is_empty() {
            rows.push(row);
        }
    }
    if rows.is_empty() {
        return Err("no numbers found — provide a table of observed counts".into());
    }
    let width = rows[0].len();
    if rows.iter().any(|r| r.len() != width) {
        return Err("every row must have the same number of columns".into());
    }
    Ok(rows)
}

// ---- public API ----------------------------------------------------------

/// Goodness-of-fit test. `observed` is a list of counts. `expected` is optional:
/// when empty, an equal-frequency (uniform) null is used; otherwise it must have
/// the same length and is treated as expected counts or ratios — if it does not
/// sum to the observed total it is rescaled to match (so ratios like `1 1 1`
/// work as well as raw counts).
pub fn goodness_of_fit(observed: &str, expected: &str) -> Result<ChiSquareResult, String> {
    let obs = parse_row(observed)?;
    if obs.len() < 2 {
        return Err("provide at least 2 observed categories".into());
    }
    if obs.iter().any(|&v| v < 0.0) {
        return Err("observed counts must be non-negative".into());
    }
    let total: f64 = obs.iter().sum();
    if total <= 0.0 {
        return Err("observed counts sum to zero".into());
    }

    let exp_in = parse_row(expected)?;
    let exp: Vec<f64> = if exp_in.is_empty() {
        // uniform null
        let each = total / obs.len() as f64;
        vec![each; obs.len()]
    } else {
        if exp_in.len() != obs.len() {
            return Err(format!(
                "expected has {} values but observed has {} — they must match",
                exp_in.len(),
                obs.len()
            ));
        }
        if exp_in.iter().any(|&v| v <= 0.0) {
            return Err("expected values must be positive".into());
        }
        // Rescale expected (counts or ratios) so it sums to the observed total.
        let esum: f64 = exp_in.iter().sum();
        exp_in.iter().map(|&v| v / esum * total).collect()
    };

    let chi: f64 = obs
        .iter()
        .zip(&exp)
        .map(|(&o, &e)| (o - e) * (o - e) / e)
        .sum();
    let df = obs.len() - 1;
    let p = chi_square_p_value(chi, df);
    let below5 = exp.iter().filter(|&&e| e < 5.0).count();

    Ok(ChiSquareResult {
        test: "goodness-of-fit".into(),
        chi_square: round6(chi),
        degrees_of_freedom: df,
        p_value: round6(p),
        total_observed: round6(total),
        expected: exp.into_iter().map(round6).collect(),
        cells_expected_below_5: below5,
        cramers_v: None,
        low_expected_warning: below5 > 0,
        yates_correction: false,
    })
}

/// Chi-square test of independence on an r×c contingency table of observed
/// counts. Lines = rows, cells separated by spaces/commas/semicolons/tabs.
/// When `yates` is true and the table is 2×2, Yates' continuity correction is
/// applied: each |O−E| is reduced by 0.5 before squaring.
pub fn contingency_with(table: &str, yates: bool) -> Result<ChiSquareResult, String> {
    let rows = parse_table(table)?;
    let r = rows.len();
    let c = rows[0].len();
    if r < 2 || c < 2 {
        return Err("a contingency table needs at least 2 rows and 2 columns".into());
    }
    if rows.iter().flatten().any(|&v| v < 0.0) {
        return Err("observed counts must be non-negative".into());
    }

    let row_totals: Vec<f64> = rows.iter().map(|row| row.iter().sum()).collect();
    let col_totals: Vec<f64> = (0..c)
        .map(|j| rows.iter().map(|row| row[j]).sum())
        .collect();
    let grand: f64 = row_totals.iter().sum();
    if grand <= 0.0 {
        return Err("table totals to zero".into());
    }

    // Yates' continuity correction only applies to a 2×2 table.
    let apply_yates = yates && r == 2 && c == 2;
    let mut expected = Vec::with_capacity(r * c);
    let mut chi = 0.0;
    let mut below5 = 0;
    for i in 0..r {
        for j in 0..c {
            let e = row_totals[i] * col_totals[j] / grand;
            if e <= 0.0 {
                return Err(
                    "a row or column total is zero — remove empty rows/columns".into(),
                );
            }
            if e < 5.0 {
                below5 += 1;
            }
            let o = rows[i][j];
            let diff = if apply_yates {
                ((o - e).abs() - 0.5).max(0.0)
            } else {
                (o - e).abs()
            };
            chi += diff * diff / e;
            expected.push(e);
        }
    }
    let df = (r - 1) * (c - 1);
    let p = chi_square_p_value(chi, df);

    // Cramér's V = sqrt( χ² / (N · (min(r,c) − 1)) )
    let k = (r.min(c) - 1) as f64;
    let cramers_v = (chi / (grand * k)).sqrt();

    Ok(ChiSquareResult {
        test: "contingency".into(),
        chi_square: round6(chi),
        degrees_of_freedom: df,
        p_value: round6(p),
        total_observed: round6(grand),
        expected: expected.into_iter().map(round6).collect(),
        cells_expected_below_5: below5,
        cramers_v: Some(round6(cramers_v)),
        low_expected_warning: below5 > 0,
        yates_correction: apply_yates,
    })
}

/// Contingency test without Yates' correction (back-compat convenience).
pub fn contingency(table: &str) -> Result<ChiSquareResult, String> {
    contingency_with(table, false)
}

fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on" | "y"
    )
}

/// Dispatch on mode: "goodness-of-fit" (default) or "contingency"/"independence".
/// `yates` (e.g. "true") enables Yates' continuity correction for 2×2 tables.
pub fn run(observed: &str, expected: &str, mode: &str, yates: &str) -> Result<ChiSquareResult, String> {
    match mode.trim().to_ascii_lowercase().as_str() {
        "" | "goodness-of-fit" | "goodness" | "gof" => goodness_of_fit(observed, expected),
        "contingency" | "independence" | "table" => contingency_with(observed, truthy(yates)),
        other => Err(format!(
            "unknown mode '{other}' — use 'goodness-of-fit' or 'contingency'"
        )),
    }
}

/// Human-readable summary (used by the page).
pub fn summary(observed: &str, expected: &str, mode: &str, yates: &str) -> Result<String, String> {
    let r = run(observed, expected, mode, yates)?;
    let sig = if r.p_value.is_nan() {
        "n/a".to_string()
    } else if r.p_value < 0.05 {
        format!("p = {} < 0.05 → reject the null hypothesis", r.p_value)
    } else {
        format!("p = {} ≥ 0.05 → fail to reject the null hypothesis", r.p_value)
    };
    let mut out = format!(
        "test: {}\nchi-square: {}\ndegrees of freedom: {}\np-value: {}\ntotal observed: {}\n{}",
        r.test, r.chi_square, r.degrees_of_freedom, r.p_value, r.total_observed, sig
    );
    if let Some(v) = r.cramers_v {
        out.push_str(&format!("\nCramér's V (effect size): {v}"));
    }
    if r.yates_correction {
        out.push_str("\nYates' continuity correction: applied (2×2 table)");
    }
    out.push_str(&format!(
        "\nexpected counts: {}",
        r.expected
            .iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    ));
    if r.low_expected_warning {
        out.push_str(&format!(
            "\nwarning: {} cell(s) have an expected count below 5 — the chi-square approximation may be unreliable",
            r.cells_expected_below_5
        ));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f64, b: f64, tol: f64) {
        assert!((a - b).abs() < tol, "{a} vs {b} (tol {tol})");
    }

    #[test]
    fn gof_uniform_die() {
        // Fair-die test: observed [16,15,19,17,11,13] expected 91/6 each.
        // scipy.stats.chisquare gives chi2 ≈ 2.692308, p ≈ 0.747295.
        let r = goodness_of_fit("16 15 19 17 11 13", "").unwrap();
        assert_eq!(r.test, "goodness-of-fit");
        assert_eq!(r.degrees_of_freedom, 5);
        approx(r.chi_square, 2.692308, 1e-4);
        approx(r.p_value, 0.747295, 1e-4);
        assert_eq!(r.total_observed, 91.0);
        assert!(!r.low_expected_warning);
    }

    #[test]
    fn gof_explicit_ratios() {
        // observed [10,20,30,40], expected ratios 1:1:1:1 → uniform 25 each.
        // chi2 = (15²+5²+5²+15²)/25 = (225+25+25+225)/25 = 500/25 = 20, df=3
        let r = goodness_of_fit("10 20 30 40", "1 1 1 1").unwrap();
        approx(r.chi_square, 20.0, 1e-9);
        assert_eq!(r.degrees_of_freedom, 3);
        // p-value for chi2=20, df=3 ≈ 0.000170
        approx(r.p_value, 0.000170, 1e-4);
    }

    #[test]
    fn gof_explicit_counts() {
        // Mendelian 9:3:3:1, n=556 (classic Fisher). observed 315,108,101,32.
        // chi2 ≈ 0.470024, df=3, p ≈ 0.925426.
        let r = goodness_of_fit("315 108 101 32", "9 3 3 1").unwrap();
        approx(r.chi_square, 0.470024, 1e-4);
        assert_eq!(r.degrees_of_freedom, 3);
        approx(r.p_value, 0.925426, 1e-4);
    }

    #[test]
    fn contingency_2x2() {
        // Classic 2x2: [[10,20],[30,40]].
        // chi2 (no Yates) ≈ 0.793651, df=1, p ≈ 0.372998.
        let r = contingency("10 20\n30 40").unwrap();
        assert_eq!(r.test, "contingency");
        assert_eq!(r.degrees_of_freedom, 1);
        approx(r.chi_square, 0.793651, 1e-4);
        approx(r.p_value, 0.372998, 1e-3);
        assert_eq!(r.total_observed, 100.0);
        assert!(r.cramers_v.is_some());
    }

    #[test]
    fn contingency_3x3_independence() {
        // A table with perfect association.
        let r = contingency("100 0 0\n0 100 0\n0 0 100").unwrap();
        assert_eq!(r.degrees_of_freedom, 4);
        // perfect association: chi2 = N*(min(r,c)-1) = 300*2 = 600
        approx(r.chi_square, 600.0, 1e-6);
        approx(r.cramers_v.unwrap(), 1.0, 1e-6);
        assert!(r.p_value < 1e-9);
    }

    #[test]
    fn run_mode_dispatch() {
        assert_eq!(run("10 20\n30 40", "", "contingency", "").unwrap().test, "contingency");
        assert_eq!(run("1 2 3", "", "", "").unwrap().test, "goodness-of-fit");
        assert!(run("1 2", "", "bogus", "").is_err());
    }

    #[test]
    fn yates_correction_2x2() {
        // [[10,20],[30,40]] with Yates: each |O-E|=2 → reduced to 1.5.
        // chi2 = 4 * 1.5²/E with E=[12,18,28,42] → R's chisq.test(...,correct=TRUE)
        // gives X-squared ≈ 0.4464, p ≈ 0.504.
        let r = run("10 20\n30 40", "", "contingency", "true").unwrap();
        assert!(r.yates_correction);
        approx(r.chi_square, 0.446429, 1e-4);
        approx(r.p_value, 0.504004, 1e-3);
        // Uncorrected must differ and be larger.
        let plain = contingency("10 20\n30 40").unwrap();
        assert!(!plain.yates_correction);
        assert!(plain.chi_square > r.chi_square);
    }

    #[test]
    fn yates_ignored_for_non_2x2() {
        // Yates only applies to 2×2; a 2×3 must report yates_correction=false.
        let r = run("10 20 30\n30 40 50", "", "contingency", "true").unwrap();
        assert!(!r.yates_correction);
    }

    #[test]
    fn p_value_known_points() {
        // chi2 = 3.841459 at df=1 → p ≈ 0.05 (the 95% critical value).
        approx(chi_square_p_value(3.841459, 1), 0.05, 1e-4);
        // chi2 = 5.991465 at df=2 → p ≈ 0.05.
        approx(chi_square_p_value(5.991465, 2), 0.05, 1e-4);
        // chi2 = 11.070498 at df=5 → p ≈ 0.05.
        approx(chi_square_p_value(11.070498, 5), 0.05, 1e-4);
        // zero stat → p = 1
        assert_eq!(chi_square_p_value(0.0, 3), 1.0);
    }

    #[test]
    fn low_expected_warning_flags() {
        // observed with tiny categories → expected < 5
        let r = goodness_of_fit("1 1 1 1", "").unwrap();
        assert!(r.low_expected_warning);
        assert_eq!(r.cells_expected_below_5, 4);
    }

    #[test]
    fn errors() {
        assert!(goodness_of_fit("", "").is_err());
        assert!(goodness_of_fit("5", "").is_err()); // need >=2 categories
        assert!(goodness_of_fit("1 2 3", "1 2").is_err()); // length mismatch
        assert!(contingency("1 2 3").is_err()); // single row
        assert!(contingency("1 2\n3").is_err()); // ragged
        assert!(goodness_of_fit("-1 2 3", "").is_err()); // negative
    }
}
