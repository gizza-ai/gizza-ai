//! multiple-regression core — ordinary-least-squares multiple linear regression.
//!
//! Given a data matrix (rows = observations, columns = variables), fit
//! `y = b0 + b1·x1 + … + bk·xk` by OLS and report, for every coefficient, its
//! estimate, standard error, t-statistic, two-tailed p-value and confidence
//! interval, plus the model-level R², adjusted R², residual standard error and
//! the overall F-test. Pure-Rust, dependency-free besides serde.
//!
//! Math: normal equations (XᵀX)b = Xᵀy solved by Gauss-Jordan with partial
//! pivoting; (XᵀX)⁻¹ is reused for the coefficient covariance σ²·(XᵀX)⁻¹.
//! t / F tail probabilities come from the regularized incomplete beta function
//! (Numerical-Recipes `betacf` continued fraction + `gammln`) — deterministic,
//! no RNG, so it instantiates identically under wasmi and native.

use serde::Serialize;

/// One fitted coefficient (the intercept, or one predictor).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Coefficient {
    /// Term name — `(Intercept)` or the predictor's column label.
    pub name: String,
    /// Estimated coefficient (the slope for a predictor).
    pub estimate: f64,
    /// Standard error of the estimate.
    pub std_error: f64,
    /// t-statistic = estimate / std_error.
    pub t_stat: f64,
    /// Two-tailed p-value for H0: coefficient = 0.
    pub p_value: f64,
    /// Lower bound of the confidence interval (at `conf_level`).
    pub ci_low: f64,
    /// Upper bound of the confidence interval (at `conf_level`).
    pub ci_high: f64,
}

/// Full regression result.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Regression {
    /// Number of observations (rows) used.
    pub n: usize,
    /// Number of predictor variables (excludes the intercept).
    pub predictors: usize,
    /// Whether an intercept (constant) term was fitted.
    pub intercept: bool,
    /// Confidence level used for the coefficient intervals (e.g. 0.95).
    pub conf_level: f64,
    /// Name of the response (dependent) variable column.
    pub response: String,
    /// The fitted regression equation as a human-readable string.
    pub equation: String,
    /// Estimated coefficients (intercept first when present, then predictors).
    pub coefficients: Vec<Coefficient>,
    /// R² — coefficient of determination.
    pub r_squared: f64,
    /// Adjusted R² (penalises extra predictors).
    pub adj_r_squared: f64,
    /// Residual standard error = √(RSS / df_residual).
    pub residual_std_error: f64,
    /// Residual degrees of freedom = n − parameters.
    pub df_residual: usize,
    /// Overall F-statistic (model vs. intercept-only / zero model).
    pub f_statistic: f64,
    /// Numerator degrees of freedom for the F-test.
    pub f_df1: usize,
    /// Denominator degrees of freedom for the F-test (= df_residual).
    pub f_df2: usize,
    /// p-value for the overall F-test (right-tailed).
    pub f_p_value: f64,
    /// Residual sum of squares.
    pub rss: f64,
    /// Total sum of squares (centered when an intercept is fitted).
    pub tss: f64,
    /// Fitted (predicted) response for each observation, in input order.
    pub fitted: Vec<f64>,
    /// Residual (observed − fitted) for each observation, in input order.
    pub residuals: Vec<f64>,
}

fn round6(v: f64) -> f64 {
    if !v.is_finite() {
        return v;
    }
    (v * 1e6).round() / 1e6
}

/// Parse a data matrix: one observation per non-blank line, columns split on
/// commas, tabs, semicolons or runs of whitespace. Every row must have the same
/// number of columns.
fn parse_matrix(text: &str) -> Result<Vec<Vec<f64>>, String> {
    let mut rows: Vec<Vec<f64>> = Vec::new();
    for (li, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let mut row = Vec::new();
        for tok in trimmed.split(|c: char| c.is_whitespace() || c == ',' || c == ';') {
            let t = tok.trim();
            if t.is_empty() {
                continue;
            }
            let v: f64 = t
                .parse()
                .map_err(|_| format!("row {}: '{}' is not a number", li + 1, t))?;
            if !v.is_finite() {
                return Err(format!("row {}: '{}' is not a finite number", li + 1, t));
            }
            row.push(v);
        }
        if !row.is_empty() {
            rows.push(row);
        }
    }
    if rows.is_empty() {
        return Err("no data found — paste rows of numbers, one observation per line".into());
    }
    let ncol = rows[0].len();
    if ncol < 2 {
        return Err(format!(
            "need at least 2 columns (≥1 predictor + 1 response); row 1 has {ncol}"
        ));
    }
    for (i, r) in rows.iter().enumerate() {
        if r.len() != ncol {
            return Err(format!(
                "row {} has {} columns but row 1 has {} — every row must have the same number of columns",
                i + 1,
                r.len(),
                ncol
            ));
        }
    }
    Ok(rows)
}

/// Resolve the `response` selector to a 0-based column index.
fn resolve_response(spec: &str, ncol: usize) -> Result<usize, String> {
    let s = spec.trim();
    match s.to_ascii_lowercase().as_str() {
        "" | "last" => Ok(ncol - 1),
        "first" => Ok(0),
        _ => {
            let idx: usize = s.parse().map_err(|_| {
                format!("response must be 'last', 'first' or a 1-based column number, got '{s}'")
            })?;
            if idx < 1 || idx > ncol {
                return Err(format!(
                    "response column {idx} is out of range 1..={ncol}"
                ));
            }
            Ok(idx - 1)
        }
    }
}

/// Column labels: parse the comma-separated `labels`, or default to v1..vN.
fn column_labels(labels: &str, ncol: usize) -> Result<Vec<String>, String> {
    let t = labels.trim();
    if t.is_empty() {
        return Ok((1..=ncol).map(|i| format!("v{i}")).collect());
    }
    let names: Vec<String> = t.split(',').map(|s| s.trim().to_string()).collect();
    if names.len() != ncol {
        return Err(format!(
            "labels has {} names but the data has {} columns",
            names.len(),
            ncol
        ));
    }
    if names.iter().any(|n| n.is_empty()) {
        return Err("labels must not contain empty names".into());
    }
    Ok(names)
}

// ---- special functions (Numerical Recipes) ----------------------------------

fn gammln(x: f64) -> f64 {
    const COF: [f64; 6] = [
        76.180_091_729_471_46,
        -86.505_320_329_416_77,
        24.014_098_240_830_91,
        -1.231_739_572_450_155,
        0.120_865_097_386_617_9e-2,
        -0.539_523_938_495_3e-5,
    ];
    let mut y = x;
    let mut tmp = x + 5.5;
    tmp -= (x + 0.5) * tmp.ln();
    let mut ser = 1.000_000_000_190_015;
    for c in COF {
        y += 1.0;
        ser += c / y;
    }
    -tmp + (2.506_628_274_631_000_5 * ser / x).ln()
}

fn betacf(a: f64, b: f64, x: f64) -> f64 {
    const MAXIT: usize = 200;
    const EPS: f64 = 3.0e-12;
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
    let bt = (gammln(a + b) - gammln(a) - gammln(b) + a * x.ln() + b * (1.0 - x).ln()).exp();
    if x < (a + 1.0) / (a + b + 2.0) {
        bt * betacf(a, b, x) / a
    } else {
        1.0 - bt * betacf(b, a, 1.0 - x) / b
    }
}

/// Two-tailed Student-t p-value: P(|T| ≥ |t|) with `df` degrees of freedom.
fn student_t_two_tail(t: f64, df: f64) -> f64 {
    if !t.is_finite() {
        return 0.0;
    }
    let x = df / (df + t * t);
    betai(df / 2.0, 0.5, x)
}

/// Right-tail F p-value: P(F ≥ f) with (d1, d2) degrees of freedom.
fn f_upper_tail(f: f64, d1: f64, d2: f64) -> f64 {
    if !f.is_finite() {
        return 0.0;
    }
    if f <= 0.0 {
        return 1.0;
    }
    let x = d2 / (d2 + d1 * f);
    betai(d2 / 2.0, d1 / 2.0, x)
}

/// Two-tailed Student-t critical value for tail probability `alpha` (bisection
/// on the monotone tail function). Returns 0 for alpha ≥ 1 or non-positive df.
fn student_t_crit(alpha: f64, df: f64) -> f64 {
    if df <= 0.0 || alpha >= 1.0 {
        return 0.0;
    }
    if alpha <= 0.0 {
        return f64::INFINITY;
    }
    let (mut lo, mut hi) = (0.0f64, 1.0e6f64);
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if student_t_two_tail(mid, df) > alpha {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

// ---- linear algebra ---------------------------------------------------------

/// Invert a square matrix by Gauss-Jordan elimination with partial pivoting.
fn invert(mat: &[Vec<f64>]) -> Result<Vec<Vec<f64>>, String> {
    let n = mat.len();
    // Augmented [A | I].
    let mut m: Vec<Vec<f64>> = Vec::with_capacity(n);
    for (i, row) in mat.iter().enumerate() {
        let mut r = row.clone();
        r.extend((0..n).map(|j| if i == j { 1.0 } else { 0.0 }));
        m.push(r);
    }
    for col in 0..n {
        // pivot
        let mut piv = col;
        for r in (col + 1)..n {
            if m[r][col].abs() > m[piv][col].abs() {
                piv = r;
            }
        }
        if m[piv][col].abs() < 1e-12 {
            return Err("predictors are perfectly collinear (or there are too few observations) — the model is not identifiable".into());
        }
        m.swap(col, piv);
        let d = m[col][col];
        for v in m[col].iter_mut() {
            *v /= d;
        }
        for r in 0..n {
            if r != col {
                let f = m[r][col];
                if f != 0.0 {
                    for k in 0..(2 * n) {
                        m[r][k] -= f * m[col][k];
                    }
                }
            }
        }
    }
    Ok(m.iter().map(|row| row[n..].to_vec()).collect())
}

/// Fit an OLS multiple linear regression.
///
/// * `data` — rows of numbers (see [`parse_matrix`]).
/// * `response` — which column is Y: `last` (default), `first`, or a 1-based index.
/// * `labels` — optional comma-separated column names (defaults v1..vN).
/// * `intercept` — fit a constant term (default true).
/// * `conf_level` — confidence level for the coefficient intervals, in (0, 1).
pub fn fit(
    data: &str,
    response: &str,
    labels: &str,
    intercept: bool,
    conf_level: f64,
) -> Result<Regression, String> {
    if !(conf_level > 0.0 && conf_level < 1.0) {
        return Err(format!(
            "conf_level must be between 0 and 1 (exclusive), got {conf_level}"
        ));
    }
    let rows = parse_matrix(data)?;
    let n = rows.len();
    let ncol = rows[0].len();
    let resp_idx = resolve_response(response, ncol)?;
    let names = column_labels(labels, ncol)?;

    // Split into predictors (all columns except the response) and y.
    let pred_idx: Vec<usize> = (0..ncol).filter(|&c| c != resp_idx).collect();
    let k = pred_idx.len();
    let y: Vec<f64> = rows.iter().map(|r| r[resp_idx]).collect();

    // Design matrix X (n × p).
    let p = k + usize::from(intercept);
    if n <= p {
        return Err(format!(
            "need more observations than parameters: {n} rows, {p} parameter(s) (residual df = {} ≤ 0). Add more rows or drop predictors.",
            n as i64 - p as i64
        ));
    }
    let mut x: Vec<Vec<f64>> = Vec::with_capacity(n);
    for r in &rows {
        let mut row = Vec::with_capacity(p);
        if intercept {
            row.push(1.0);
        }
        for &c in &pred_idx {
            row.push(r[c]);
        }
        x.push(row);
    }

    // Normal equations: XtX b = Xty.
    let mut xtx = vec![vec![0.0f64; p]; p];
    let mut xty = vec![0.0f64; p];
    for row_i in 0..n {
        for i in 0..p {
            xty[i] += x[row_i][i] * y[row_i];
            for j in 0..p {
                xtx[i][j] += x[row_i][i] * x[row_i][j];
            }
        }
    }
    let inv = invert(&xtx)?;
    let b: Vec<f64> = (0..p)
        .map(|i| (0..p).map(|j| inv[i][j] * xty[j]).sum())
        .collect();

    // Fitted, residuals, RSS.
    let fitted: Vec<f64> = (0..n)
        .map(|r| (0..p).map(|i| x[r][i] * b[i]).sum())
        .collect();
    let residuals: Vec<f64> = (0..n).map(|r| y[r] - fitted[r]).collect();
    let rss: f64 = residuals.iter().map(|e| e * e).sum();

    // Total sum of squares: centered if an intercept is present, else uncentered.
    let ybar = y.iter().sum::<f64>() / n as f64;
    let tss: f64 = if intercept {
        y.iter().map(|v| (v - ybar) * (v - ybar)).sum()
    } else {
        y.iter().map(|v| v * v).sum()
    };

    let df_resid = n - p;
    let sigma2 = rss / df_resid as f64;
    let rse = sigma2.max(0.0).sqrt();

    let r2 = if tss == 0.0 {
        if rss <= 1e-12 {
            1.0
        } else {
            0.0
        }
    } else {
        1.0 - rss / tss
    };
    let df_tot = if intercept { n - 1 } else { n };
    let adj = 1.0 - (1.0 - r2) * (df_tot as f64) / (df_resid as f64);

    // Per-coefficient inference.
    let alpha = 1.0 - conf_level;
    let tcrit = student_t_crit(alpha, df_resid as f64);
    let mut coeff_names: Vec<String> = Vec::with_capacity(p);
    if intercept {
        coeff_names.push("(Intercept)".to_string());
    }
    for &c in &pred_idx {
        coeff_names.push(names[c].clone());
    }
    let mut coefficients = Vec::with_capacity(p);
    for i in 0..p {
        let se = (sigma2 * inv[i][i]).max(0.0).sqrt();
        let (t_stat, p_value) = if se == 0.0 {
            if b[i] == 0.0 {
                (0.0, 1.0)
            } else {
                (f64::INFINITY, 0.0)
            }
        } else {
            let t = b[i] / se;
            (t, student_t_two_tail(t, df_resid as f64))
        };
        coefficients.push(Coefficient {
            name: coeff_names[i].clone(),
            estimate: round6(b[i]),
            std_error: round6(se),
            t_stat: round6(t_stat),
            p_value: round6(p_value),
            ci_low: round6(b[i] - tcrit * se),
            ci_high: round6(b[i] + tcrit * se),
        });
    }

    // Overall F-test.
    let df1 = k; // number of predictors
    let (f_stat, f_p) = if df1 == 0 {
        (f64::NAN, f64::NAN)
    } else if rss <= 1e-12 {
        (f64::INFINITY, 0.0)
    } else {
        let f = ((tss - rss) / df1 as f64) / (rss / df_resid as f64);
        (f, f_upper_tail(f, df1 as f64, df_resid as f64))
    };

    // Human-readable equation.
    let equation = build_equation(&coeff_names, &b, &names[resp_idx], intercept);

    Ok(Regression {
        n,
        predictors: k,
        intercept,
        conf_level,
        response: names[resp_idx].clone(),
        equation,
        coefficients,
        r_squared: round6(r2),
        adj_r_squared: round6(adj),
        residual_std_error: round6(rse),
        df_residual: df_resid,
        f_statistic: round6(f_stat),
        f_df1: df1,
        f_df2: df_resid,
        f_p_value: round6(f_p),
        rss: round6(rss),
        tss: round6(tss),
        fitted: fitted.into_iter().map(round6).collect(),
        residuals: residuals.into_iter().map(round6).collect(),
    })
}

fn fmt_num(v: f64) -> String {
    let r = round6(v);
    // trim trailing zeros for a tidy equation, keep up to 6 dp.
    let s = format!("{r:.6}");
    let s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    if s.is_empty() || s == "-0" {
        "0".to_string()
    } else {
        s
    }
}

fn build_equation(coeff_names: &[String], b: &[f64], response: &str, intercept: bool) -> String {
    let mut eq = format!("{response} = ");
    let mut first = true;
    for (i, name) in coeff_names.iter().enumerate() {
        let v = b[i];
        if intercept && i == 0 {
            eq.push_str(&fmt_num(v));
            first = false;
            continue;
        }
        let sign = if v < 0.0 { " − " } else { " + " };
        if first {
            if v < 0.0 {
                eq.push('−');
            }
            first = false;
        } else {
            eq.push_str(sign);
        }
        eq.push_str(&format!("{}·{}", fmt_num(v.abs()), name));
    }
    eq
}

/// Human-readable summary used by the web page (`format = "text"`).
pub fn summary(
    data: &str,
    response: &str,
    labels: &str,
    intercept: bool,
    conf_level: f64,
) -> Result<String, String> {
    let r = fit(data, response, labels, intercept, conf_level)?;
    let pct = round6(conf_level * 100.0);
    let mut out = String::new();
    out.push_str(&format!("{}\n\n", r.equation));
    out.push_str("Coefficients:\n");
    out.push_str(&format!(
        "  {:<14} {:>12} {:>12} {:>10} {:>10}   {:.0}% CI\n",
        "term", "estimate", "std error", "t", "p", pct
    ));
    for c in &r.coefficients {
        out.push_str(&format!(
            "  {:<14} {:>12} {:>12} {:>10} {:>10}   [{}, {}]\n",
            c.name, c.estimate, c.std_error, c.t_stat, c.p_value, c.ci_low, c.ci_high
        ));
    }
    out.push('\n');
    out.push_str(&format!("Observations: {}\n", r.n));
    out.push_str(&format!("Predictors: {}\n", r.predictors));
    out.push_str(&format!("R-squared: {}\n", r.r_squared));
    out.push_str(&format!("Adjusted R-squared: {}\n", r.adj_r_squared));
    out.push_str(&format!(
        "Residual std error: {} on {} degrees of freedom\n",
        r.residual_std_error, r.df_residual
    ));
    out.push_str(&format!(
        "F-statistic: {} on {} and {} DF,  p-value: {}\n",
        r.f_statistic, r.f_df1, r.f_df2, r.f_p_value
    ));
    Ok(out)
}

/// Entry point shared by every surface (chat, CLI, web page). Fits the model
/// and renders either the human-readable `summary` (`format = "text"`, default)
/// or the full [`Regression`] struct as pretty JSON (`format = "json"`, which
/// additionally exposes the per-observation `fitted`/`residuals` arrays).
pub fn run(
    data: &str,
    response: &str,
    labels: &str,
    intercept: bool,
    conf_level: f64,
    format: &str,
) -> Result<String, String> {
    match format.trim().to_ascii_lowercase().as_str() {
        "" | "text" => summary(data, response, labels, intercept, conf_level),
        "json" => {
            let r = fit(data, response, labels, intercept, conf_level)?;
            serde_json::to_string_pretty(&r).map_err(|e| e.to_string())
        }
        other => Err(format!("format must be 'text' or 'json', got '{other}'")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() <= tol
    }

    #[test]
    fn simple_regression_matches_hand_computation() {
        // x=[1..5], y=[2,4,5,4,5]: intercept 2.2, slope 0.6, R²=0.6, F=4.5.
        let r = fit("1 2\n2 4\n3 5\n4 4\n5 5", "last", "", true, 0.95).unwrap();
        assert_eq!(r.n, 5);
        assert_eq!(r.predictors, 1);
        assert_eq!(r.df_residual, 3);
        assert!(close(r.coefficients[0].estimate, 2.2, 1e-6));
        assert!(close(r.coefficients[1].estimate, 0.6, 1e-6));
        assert!(close(r.r_squared, 0.6, 1e-6));
        assert!(close(r.adj_r_squared, 0.466667, 1e-6));
        assert!(close(r.residual_std_error, 0.894427, 1e-6));
        assert!(close(r.f_statistic, 4.5, 1e-6));
        assert_eq!(r.f_df1, 1);
        assert_eq!(r.f_df2, 3);
        // std errors / t / p from the (XᵀX)⁻¹ path.
        assert!(close(r.coefficients[0].std_error, 0.938083, 1e-5));
        assert!(close(r.coefficients[1].std_error, 0.282843, 1e-5));
        assert!(close(r.coefficients[1].t_stat, 2.12132, 1e-4));
        assert!(close(r.coefficients[1].p_value, 0.124027, 1e-4));
        assert!(close(r.f_p_value, 0.124027, 1e-4));
    }

    #[test]
    fn two_predictor_regression() {
        // y = 2 + 2.333x1 + 1.333x2 (approx), from the python reference.
        let data = "1 1 6\n2 1 8\n3 2 11\n4 2 14\n5 3 18\n6 3 20";
        let r = fit(data, "last", "sqft, rooms, price", true, 0.95).unwrap();
        assert_eq!(r.predictors, 2);
        assert_eq!(r.response, "price");
        assert!(close(r.coefficients[0].estimate, 2.0, 1e-6));
        assert!(close(r.coefficients[1].estimate, 2.333333, 1e-6));
        assert!(close(r.coefficients[2].estimate, 1.333333, 1e-6));
        assert!(close(r.coefficients[1].std_error, 0.3849, 1e-4));
        assert!(close(r.coefficients[1].t_stat, 6.062178, 1e-3));
        assert!(close(r.coefficients[1].p_value, 0.009007, 1e-4));
        assert!(close(r.coefficients[2].p_value, 0.196267, 1e-4));
        assert!(close(r.r_squared, 0.995638, 1e-6));
        assert!(close(r.adj_r_squared, 0.99273, 1e-5));
        assert!(close(r.f_statistic, 342.375, 1e-2));
        assert!(close(r.f_p_value, 0.000288, 1e-5));
        // named predictors appear in the coefficient table.
        assert_eq!(r.coefficients[1].name, "sqft");
        assert_eq!(r.coefficients[2].name, "rooms");
    }

    #[test]
    fn response_selector_first_column() {
        // Same data, but declare column 1 (price-ish) as the response.
        let r = fit("6 1 1\n8 1 2\n11 2 3\n14 2 4", "first", "", true, 0.95).unwrap();
        assert_eq!(r.response, "v1");
        assert_eq!(r.predictors, 2);
    }

    #[test]
    fn no_intercept_option() {
        let r = fit("1 2\n2 4.1\n3 5.9\n4 8.2", "last", "", false, 0.95).unwrap();
        assert!(!r.intercept);
        assert_eq!(r.coefficients.len(), 1);
        assert_ne!(r.coefficients[0].name, "(Intercept)");
        assert!(close(r.coefficients[0].estimate, 2.023333, 1e-5));
    }

    #[test]
    fn confidence_interval_brackets_estimate() {
        let r = fit("1 2\n2 4\n3 5\n4 4\n5 5", "last", "", true, 0.95).unwrap();
        let c = &r.coefficients[1];
        assert!(c.ci_low < c.estimate && c.estimate < c.ci_high);
        assert!(close(c.ci_low, -0.300132, 1e-4));
        assert!(close(c.ci_high, 1.500132, 1e-4));
    }

    #[test]
    fn errors_on_bad_input() {
        assert!(fit("", "last", "", true, 0.95).is_err());
        assert!(fit("1 2\n3 abc", "last", "", true, 0.95).is_err());
        // ragged rows
        assert!(fit("1 2\n3 4 5", "last", "", true, 0.95).is_err());
        // single column (no predictor)
        assert!(fit("1\n2\n3", "last", "", true, 0.95).is_err());
        // too few observations for the parameters (2 params, 2 rows)
        assert!(fit("1 2\n3 4", "last", "", true, 0.95).is_err());
        // perfect collinearity between predictors
        assert!(fit("1 2 5\n2 4 7\n3 6 9\n4 8 12", "last", "", true, 0.95).is_err());
        // bad conf_level
        assert!(fit("1 2\n2 4\n3 5", "last", "", true, 1.5).is_err());
        // out-of-range response index
        assert!(fit("1 2\n2 4\n3 5", "9", "", true, 0.95).is_err());
        // wrong label count
        assert!(fit("1 2\n2 4\n3 5", "last", "a,b,c", true, 0.95).is_err());
    }

    #[test]
    fn summary_renders_equation_and_stats() {
        let s = summary("1 2\n2 4\n3 5\n4 4\n5 5", "last", "", true, 0.95).unwrap();
        assert!(s.contains("R-squared: 0.6"));
        assert!(s.contains("F-statistic: 4.5"));
        assert!(s.contains("(Intercept)"));
    }

    #[test]
    fn run_text_matches_summary() {
        let data = "1 2\n2 4\n3 5\n4 4\n5 5";
        let text = run(data, "last", "", true, 0.95, "text").unwrap();
        assert_eq!(text, summary(data, "last", "", true, 0.95).unwrap());
        // empty format defaults to text
        assert_eq!(run(data, "last", "", true, 0.95, "").unwrap(), text);
    }

    #[test]
    fn run_json_has_structured_fields() {
        let data = "1 2\n2 4\n3 5\n4 4\n5 5";
        let json = run(data, "last", "", true, 0.95, "json").unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["n"], 5);
        assert_eq!(v["r_squared"], 0.6);
        assert_eq!(v["coefficients"][0]["name"], "(Intercept)");
        assert_eq!(v["coefficients"][0]["estimate"], 2.2);
        assert!(v["fitted"].as_array().unwrap().len() == 5);
        assert!(v["residuals"].as_array().unwrap().len() == 5);
    }

    #[test]
    fn run_rejects_unknown_format() {
        assert!(run("1 2\n2 4\n3 5", "last", "", true, 0.95, "xml").is_err());
    }
}
