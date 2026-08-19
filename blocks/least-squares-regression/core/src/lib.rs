//! least-squares-regression core — single-variable ordinary least-squares curve fitting.
//!
//! Fits `y = b0 + b1·x + b2·x² + … + bd·x^d` to pasted `(x, y)` points by
//! minimising the sum of squared residuals, and reports the fitted equation,
//! every coefficient with its standard error, R², adjusted R², RMSE, the
//! residual standard error, the residual spread, and optional predictions at
//! new x values.
//!
//! Math: the Vandermonde design matrix is column-normalised and then reduced by
//! **Householder QR** — not the normal equations, which square the condition
//! number and lose the higher-degree fits. `(XᵀX)⁻¹ = R⁻¹R⁻ᵀ` falls out of the
//! triangular factor and gives the coefficient standard errors. Everything is
//! deterministic f64 arithmetic with no RNG, so it behaves identically under
//! wasmi, wasm-bindgen and native.

use serde::Serialize;

/// Largest accepted number of `(x, y)` points.
pub const MAX_POINTS: usize = 20_000;
/// Highest accepted polynomial degree.
pub const MAX_DEGREE: i64 = 10;
/// Relative threshold on the QR diagonal below which the fit is called
/// numerically rank-deficient.
const RCOND: f64 = 1e-13;

/// One fitted term of the polynomial.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Term {
    /// Human-readable term name — `intercept`, `x`, `x²`, …
    pub name: String,
    /// Power of x this coefficient multiplies (0 = the constant term).
    pub power: usize,
    /// Estimated coefficient.
    pub estimate: f64,
    /// Standard error of the estimate.
    pub std_error: f64,
}

/// One observation with its fitted value and residual.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Point {
    pub x: f64,
    pub y: f64,
    pub fitted: f64,
    pub residual: f64,
}

/// A prediction of y at a user-supplied x.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Prediction {
    pub x: f64,
    pub y: f64,
}

/// The complete fit.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Fit {
    /// Polynomial degree that was fitted.
    pub degree: usize,
    /// Whether a constant term was included.
    pub intercept: bool,
    /// Number of points used.
    pub n: usize,
    /// Name used for the x column (from a header row, else `x`).
    pub x_label: String,
    /// Name used for the y column (from a header row, else `y`).
    pub y_label: String,
    /// The fitted equation, highest power first, e.g. `y = 3·x² + 2·x + 1`.
    pub equation: String,
    /// Fitted terms, constant first.
    pub terms: Vec<Term>,
    /// R² — the share of variance in y explained by the fit.
    pub r_squared: f64,
    /// Adjusted R², penalising extra polynomial terms.
    pub adj_r_squared: f64,
    /// Pearson correlation of x and y — only a summary of a straight-line fit,
    /// so it is `None` for degree ≥ 2.
    pub pearson_r: Option<f64>,
    /// Root mean squared error = √(RSS / n).
    pub rmse: f64,
    /// Residual standard error = √(RSS / df).
    pub residual_std_error: f64,
    /// Residual degrees of freedom = n − number of terms.
    pub df_residual: usize,
    /// Residual sum of squares.
    pub rss: f64,
    /// Total sum of squares (centred when an intercept is fitted).
    pub tss: f64,
    /// Smallest residual.
    pub residual_min: f64,
    /// Median residual.
    pub residual_median: f64,
    /// Largest residual.
    pub residual_max: f64,
    /// Every observation with its fitted value and residual, in input order.
    pub points: Vec<Point>,
    /// Predictions at the requested x values (empty when none were asked for).
    pub predictions: Vec<Prediction>,
}

// ---------------------------------------------------------------- formatting

const SUPERSCRIPTS: [char; 10] = ['⁰', '¹', '²', '³', '⁴', '⁵', '⁶', '⁷', '⁸', '⁹'];

fn superscript(power: usize) -> String {
    power
        .to_string()
        .chars()
        .map(|c| SUPERSCRIPTS[c as usize - '0' as usize])
        .collect()
}

fn term_name(x_label: &str, power: usize) -> String {
    match power {
        0 => "intercept".to_string(),
        1 => x_label.to_string(),
        p => format!("{x_label}{}", superscript(p)),
    }
}

/// Fixed-decimal rendering that never emits a signed zero.
fn fmt_num(v: f64, decimals: usize) -> String {
    if !v.is_finite() {
        return if v.is_nan() {
            "n/a".into()
        } else if v > 0.0 {
            "inf".into()
        } else {
            "-inf".into()
        };
    }
    let s = format!("{:.*}", decimals, v);
    if s.starts_with('-') && s[1..].chars().all(|c| c == '0' || c == '.') {
        return s[1..].to_string();
    }
    s
}

fn round_to(v: f64, decimals: usize) -> f64 {
    if !v.is_finite() {
        return v;
    }
    let f = 10f64.powi(decimals as i32);
    (v * f).round() / f
}

fn degree_name(degree: usize) -> &'static str {
    match degree {
        1 => "linear",
        2 => "quadratic",
        3 => "cubic",
        4 => "quartic",
        5 => "quintic",
        6 => "sextic",
        7 => "septic",
        8 => "octic",
        9 => "nonic",
        _ => "decic",
    }
}

// -------------------------------------------------------------------- parsing

fn is_separator(c: char) -> bool {
    c.is_whitespace() || c == ',' || c == ';'
}

fn split_fields(line: &str) -> Vec<&str> {
    line.split(is_separator)
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .collect()
}

fn parse_number(tok: &str, what: &str) -> Result<f64, String> {
    // Tolerate spreadsheet exports that quote cells or keep a leading '+'.
    let cleaned = tok.trim_matches(|c| c == '"' || c == '\'');
    let cleaned = cleaned.strip_prefix('+').unwrap_or(cleaned);
    let v: f64 = cleaned
        .parse()
        .map_err(|_| format!("{what}: '{tok}' is not a number"))?;
    if !v.is_finite() {
        return Err(format!("{what}: '{tok}' is not a finite number"));
    }
    Ok(v)
}

fn looks_numeric(tok: &str) -> bool {
    parse_number(tok, "").is_ok()
}

/// How to treat a leading label row/token.
#[derive(Clone, Copy, PartialEq)]
enum Header {
    Auto,
    Yes,
    No,
}

fn parse_header_mode(header: &str) -> Result<Header, String> {
    match header.trim().to_ascii_lowercase().as_str() {
        "" | "auto" => Ok(Header::Auto),
        "yes" | "true" | "1" | "on" => Ok(Header::Yes),
        "no" | "false" | "0" | "off" => Ok(Header::No),
        other => Err(format!(
            "header must be 'auto', 'yes' or 'no' (got '{other}')"
        )),
    }
}

struct Parsed {
    xs: Vec<f64>,
    ys: Vec<f64>,
    x_label: String,
    y_label: String,
}

/// Parse a flat list of numbers, optionally consuming a leading label.
fn parse_list(
    text: &str,
    mode: Header,
    default_label: &str,
    what: &str,
) -> Result<(Vec<f64>, String), String> {
    let toks: Vec<&str> = split_fields(text);
    if toks.is_empty() {
        return Err(format!("{what}: no values found"));
    }
    let mut label = default_label.to_string();
    let mut start = 0;
    let takes_header = match mode {
        Header::Yes => true,
        Header::No => false,
        Header::Auto => !looks_numeric(toks[0]),
    };
    if takes_header {
        if toks.len() < 2 {
            return Err(format!(
                "{what}: found a header ('{}') but no values after it",
                toks[0]
            ));
        }
        label = toks[0].trim().to_string();
        start = 1;
    }
    let mut out = Vec::with_capacity(toks.len() - start);
    for tok in &toks[start..] {
        out.push(parse_number(tok, what)?);
    }
    if label.is_empty() {
        label = default_label.to_string();
    }
    Ok((out, label))
}

/// Parse either two-column `(x, y)` rows, or an x list plus a separate y list.
fn parse_input(data: &str, y_values: &str, mode: Header) -> Result<Parsed, String> {
    if !y_values.trim().is_empty() {
        let (xs, x_label) = parse_list(data, mode, "x", "x values")?;
        let (ys, y_label) = parse_list(y_values, mode, "y", "y values")?;
        if xs.len() != ys.len() {
            return Err(format!(
                "x and y must have the same number of values (got {} x and {} y)",
                xs.len(),
                ys.len()
            ));
        }
        return Ok(Parsed { xs, ys, x_label, y_label });
    }

    let mut xs = Vec::new();
    let mut ys = Vec::new();
    let mut x_label = "x".to_string();
    let mut y_label = "y".to_string();
    let mut header_taken = false;

    for (li, line) in data.lines().enumerate() {
        let fields = split_fields(line);
        if fields.is_empty() {
            continue;
        }
        if fields.len() != 2 {
            return Err(format!(
                "line {}: expected 2 values (x and y), found {} — this tool fits one x against one y, so paste two columns",
                li + 1,
                fields.len()
            ));
        }
        let is_header = !header_taken
            && xs.is_empty()
            && match mode {
                Header::Yes => true,
                Header::No => false,
                Header::Auto => !(looks_numeric(fields[0]) && looks_numeric(fields[1])),
            };
        if is_header {
            header_taken = true;
            if !fields[0].trim().is_empty() {
                x_label = fields[0].trim().to_string();
            }
            if !fields[1].trim().is_empty() {
                y_label = fields[1].trim().to_string();
            }
            continue;
        }
        xs.push(parse_number(fields[0], &format!("line {}", li + 1))?);
        ys.push(parse_number(fields[1], &format!("line {}", li + 1))?);
    }

    if xs.is_empty() {
        return Err("no data points found — paste one 'x, y' pair per line (e.g. '1, 2')".into());
    }
    Ok(Parsed { xs, ys, x_label, y_label })
}

// ------------------------------------------------------------ linear algebra

fn rank_error() -> String {
    "the fit is numerically rank-deficient — the x values do not pin down a polynomial of this degree. Lower the degree, add more spread-out points, or centre x (subtract its mean) before fitting.".into()
}

/// Householder QR of the n×p matrix `a` (`a[row][col]`), applying the same
/// reflections to `b`. Returns the p×p upper-triangular `R` and leaves `Qᵀb` in
/// the first p entries of `b`.
fn householder_qr(a: &mut [Vec<f64>], b: &mut [f64], p: usize) -> Result<Vec<Vec<f64>>, String> {
    let n = a.len();
    for k in 0..p {
        let norm2: f64 = (k..n).map(|i| a[i][k] * a[i][k]).sum();
        let norm = norm2.sqrt();
        if !norm.is_finite() {
            return Err(magnitude_error());
        }
        if norm == 0.0 {
            return Err(rank_error());
        }
        let alpha = if a[k][k] > 0.0 { -norm } else { norm };
        let mut v = vec![0.0; n];
        for i in k..n {
            v[i] = a[i][k];
        }
        v[k] -= alpha;
        let vnorm2: f64 = v[k..n].iter().map(|t| t * t).sum();
        if vnorm2 > 0.0 {
            for j in k..p {
                let dot: f64 = (k..n).map(|i| v[i] * a[i][j]).sum();
                let f = 2.0 * dot / vnorm2;
                for i in k..n {
                    a[i][j] -= f * v[i];
                }
            }
            let dot: f64 = (k..n).map(|i| v[i] * b[i]).sum();
            let f = 2.0 * dot / vnorm2;
            for i in k..n {
                b[i] -= f * v[i];
            }
        }
        a[k][k] = alpha;
        for row in a.iter_mut().take(n).skip(k + 1) {
            row[k] = 0.0;
        }
    }

    let mut r = vec![vec![0.0; p]; p];
    for (i, ri) in r.iter_mut().enumerate() {
        ri[i..p].copy_from_slice(&a[i][i..p]);
    }

    let max_diag = (0..p).map(|i| r[i][i].abs()).fold(0.0f64, f64::max);
    if max_diag == 0.0 || (0..p).any(|i| r[i][i].abs() <= RCOND * max_diag) {
        return Err(rank_error());
    }
    Ok(r)
}

fn magnitude_error() -> String {
    "the design matrix overflowed — the x values are too large to raise to this power. Rescale x (e.g. subtract its mean or divide by 1000) or lower the degree.".into()
}

/// Solve `R b = rhs` for an upper-triangular R.
fn back_substitute(r: &[Vec<f64>], rhs: &[f64], p: usize) -> Vec<f64> {
    let mut out = vec![0.0; p];
    for i in (0..p).rev() {
        let mut s = rhs[i];
        for j in i + 1..p {
            s -= r[i][j] * out[j];
        }
        out[i] = s / r[i][i];
    }
    out
}

/// Invert an upper-triangular matrix by back substitution, column by column.
fn invert_upper(r: &[Vec<f64>], p: usize) -> Vec<Vec<f64>> {
    let mut inv = vec![vec![0.0; p]; p];
    for col in 0..p {
        let mut e = vec![0.0; p];
        e[col] = 1.0;
        let x = back_substitute(r, &e, p);
        for (row, xr) in x.iter().enumerate() {
            inv[row][col] = *xr;
        }
    }
    inv
}

// ------------------------------------------------------------------- fitting

/// Fit the polynomial and assemble the full result.
#[allow(clippy::too_many_arguments)]
pub fn fit(
    data: &str,
    y_values: &str,
    degree: i64,
    header: &str,
    intercept: bool,
    predict_x: &str,
    decimals: i64,
) -> Result<Fit, String> {
    if !(1..=MAX_DEGREE).contains(&degree) {
        return Err(format!(
            "degree must be between 1 and {MAX_DEGREE} (got {degree})"
        ));
    }
    if !(0..=12).contains(&decimals) {
        return Err(format!("decimals must be between 0 and 12 (got {decimals})"));
    }
    let decimals = decimals as usize;
    let degree = degree as usize;
    let mode = parse_header_mode(header)?;

    let Parsed { xs, ys, x_label, y_label } = parse_input(data, y_values, mode)?;
    let n = xs.len();
    if n > MAX_POINTS {
        return Err(format!("too many points: {n} (the limit is {MAX_POINTS})"));
    }

    let p = if intercept { degree + 1 } else { degree };
    if n <= p {
        return Err(format!(
            "not enough points: a degree-{degree} fit{} estimates {p} coefficient(s), so it needs at least {} points to leave any residual degrees of freedom (got {n})",
            if intercept { "" } else { " without an intercept" },
            p + 1
        ));
    }

    let mut distinct: Vec<f64> = xs.clone();
    distinct.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    distinct.dedup();
    if distinct.len() < p {
        return Err(format!(
            "not enough distinct x values: a degree-{degree} fit needs at least {p} different x values (got {})",
            distinct.len()
        ));
    }

    // Design matrix: powers of x, starting at 0 with an intercept, else at 1.
    let first_power = if intercept { 0 } else { 1 };
    let mut a: Vec<Vec<f64>> = xs
        .iter()
        .map(|x| {
            (0..p)
                .map(|j| x.powi((first_power + j) as i32))
                .collect::<Vec<f64>>()
        })
        .collect();

    // Column-normalise before the QR: a raw Vandermonde is badly scaled once
    // the powers differ by orders of magnitude, and the rank test below is only
    // meaningful on a balanced matrix. Coefficients are unscaled afterwards.
    let mut scales = vec![0.0f64; p];
    for (j, s) in scales.iter_mut().enumerate() {
        let norm2: f64 = a.iter().map(|row| row[j] * row[j]).sum();
        if !norm2.is_finite() {
            return Err(magnitude_error());
        }
        let norm = norm2.sqrt();
        if norm == 0.0 {
            return Err(rank_error());
        }
        *s = norm;
    }
    for row in a.iter_mut() {
        for (j, s) in scales.iter().enumerate() {
            row[j] /= s;
        }
    }

    let mut b = ys.clone();
    let r = householder_qr(&mut a, &mut b, p)?;
    let scaled = back_substitute(&r, &b, p);
    let coefs: Vec<f64> = scaled
        .iter()
        .zip(scales.iter())
        .map(|(c, s)| c / s)
        .collect();
    if coefs.iter().any(|c| !c.is_finite()) {
        return Err(rank_error());
    }

    let eval = |x: f64| -> f64 {
        coefs
            .iter()
            .enumerate()
            .map(|(j, c)| c * x.powi((first_power + j) as i32))
            .sum()
    };

    let raw_points: Vec<Point> = xs
        .iter()
        .zip(ys.iter())
        .map(|(&x, &y)| {
            let fitted = eval(x);
            Point { x, y, fitted, residual: y - fitted }
        })
        .collect();

    let rss: f64 = raw_points.iter().map(|pt| pt.residual * pt.residual).sum();
    let mean_y = ys.iter().sum::<f64>() / n as f64;
    let tss: f64 = if intercept {
        ys.iter().map(|y| (y - mean_y) * (y - mean_y)).sum()
    } else {
        ys.iter().map(|y| y * y).sum()
    };
    let r_squared = if tss > 0.0 {
        1.0 - rss / tss
    } else if rss <= f64::EPSILON {
        1.0
    } else {
        0.0
    };
    let df_residual = n - p;
    let k = if intercept { 1 } else { 0 };
    let adj_r_squared = 1.0 - (1.0 - r_squared) * ((n - k) as f64) / (df_residual as f64);
    let rmse = (rss / n as f64).sqrt();
    let sigma2 = rss / df_residual as f64;
    let residual_std_error = sigma2.sqrt();

    // (XᵀX)⁻¹ = R⁻¹R⁻ᵀ on the scaled basis; divide the standard error by the
    // same column scale that was applied to the coefficient.
    let rinv = invert_upper(&r, p);
    let terms: Vec<Term> = coefs
        .iter()
        .enumerate()
        .map(|(j, &estimate)| {
            let diag: f64 = rinv[j].iter().map(|t| t * t).sum();
            let power = first_power + j;
            Term {
                name: term_name(&x_label, power),
                power,
                estimate: round_to(estimate, decimals),
                std_error: round_to((sigma2 * diag).sqrt() / scales[j], decimals),
            }
        })
        .collect();

    // Pearson r only summarises a straight-line relationship.
    let pearson_r = if degree == 1 {
        let mean_x = xs.iter().sum::<f64>() / n as f64;
        let sxy: f64 = xs
            .iter()
            .zip(ys.iter())
            .map(|(x, y)| (x - mean_x) * (y - mean_y))
            .sum();
        let sxx: f64 = xs.iter().map(|x| (x - mean_x) * (x - mean_x)).sum();
        let syy: f64 = ys.iter().map(|y| (y - mean_y) * (y - mean_y)).sum();
        if sxx > 0.0 && syy > 0.0 {
            Some(round_to(sxy / (sxx * syy).sqrt(), decimals))
        } else {
            None
        }
    } else {
        None
    };

    let mut sorted: Vec<f64> = raw_points.iter().map(|pt| pt.residual).collect();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let residual_median = if n % 2 == 1 {
        sorted[n / 2]
    } else {
        (sorted[n / 2 - 1] + sorted[n / 2]) / 2.0
    };

    let mut predictions = Vec::new();
    for tok in split_fields(predict_x) {
        let x = parse_number(tok, "predict x")?;
        predictions.push(Prediction {
            x: round_to(x, decimals),
            y: round_to(eval(x), decimals),
        });
    }

    let equation = build_equation(&terms, &y_label, decimals);

    Ok(Fit {
        degree,
        intercept,
        n,
        x_label,
        y_label,
        equation,
        terms,
        r_squared: round_to(r_squared, decimals),
        adj_r_squared: round_to(adj_r_squared, decimals),
        pearson_r,
        rmse: round_to(rmse, decimals),
        residual_std_error: round_to(residual_std_error, decimals),
        df_residual,
        rss: round_to(rss, decimals),
        tss: round_to(tss, decimals),
        residual_min: round_to(sorted[0], decimals),
        residual_median: round_to(residual_median, decimals),
        residual_max: round_to(sorted[n - 1], decimals),
        points: raw_points
            .into_iter()
            .map(|pt| Point {
                x: round_to(pt.x, decimals),
                y: round_to(pt.y, decimals),
                fitted: round_to(pt.fitted, decimals),
                residual: round_to(pt.residual, decimals),
            })
            .collect(),
        predictions,
    })
}

/// Render the fitted polynomial highest-power first, the way calculators and
/// spreadsheet trendlines print it.
fn build_equation(terms: &[Term], y_label: &str, decimals: usize) -> String {
    let mut s = format!("{y_label} = ");
    for (i, t) in terms.iter().rev().enumerate() {
        let mag = fmt_num(t.estimate.abs(), decimals);
        let body = if t.power == 0 {
            mag
        } else {
            format!("{mag}·{}", t.name)
        };
        if i == 0 {
            if t.estimate < 0.0 {
                s.push('-');
            }
        } else if t.estimate < 0.0 {
            s.push_str(" - ");
        } else {
            s.push_str(" + ");
        }
        s.push_str(&body);
    }
    s
}

// -------------------------------------------------------------- presentation

fn render_text(f: &Fit, decimals: usize) -> String {
    let mut out = String::new();
    out.push_str(&f.equation);
    out.push_str("\n\nModel\n");
    out.push_str(&format!(
        "  fit                 {} (degree {}){}\n",
        degree_name(f.degree),
        f.degree,
        if f.intercept { "" } else { ", through the origin" }
    ));
    out.push_str(&format!("  points              {}\n", f.n));
    out.push_str(&format!(
        "  R²                  {}\n",
        fmt_num(f.r_squared, decimals)
    ));
    out.push_str(&format!(
        "  adjusted R²         {}\n",
        fmt_num(f.adj_r_squared, decimals)
    ));
    if let Some(r) = f.pearson_r {
        out.push_str(&format!("  Pearson r           {}\n", fmt_num(r, decimals)));
    }
    out.push_str(&format!(
        "  RMSE                {}\n",
        fmt_num(f.rmse, decimals)
    ));
    out.push_str(&format!(
        "  residual std error  {} on {} DF\n",
        fmt_num(f.residual_std_error, decimals),
        f.df_residual
    ));

    out.push_str("\nCoefficients\n");
    let width = f
        .terms
        .iter()
        .map(|t| t.name.chars().count())
        .max()
        .unwrap_or(4)
        .max(4);
    out.push_str(&format!("  {:<width$}  estimate  std error\n", "term"));
    for t in &f.terms {
        out.push_str(&format!(
            "  {:<width$}  {}  {}\n",
            t.name,
            fmt_num(t.estimate, decimals),
            fmt_num(t.std_error, decimals)
        ));
    }

    out.push_str("\nResiduals\n");
    out.push_str(&format!(
        "  min {}  median {}  max {}\n",
        fmt_num(f.residual_min, decimals),
        fmt_num(f.residual_median, decimals),
        fmt_num(f.residual_max, decimals)
    ));

    if !f.predictions.is_empty() {
        out.push_str("\nPredictions\n");
        for p in &f.predictions {
            out.push_str(&format!(
                "  {} = {}  ->  {} = {}\n",
                f.x_label,
                fmt_num(p.x, decimals),
                f.y_label,
                fmt_num(p.y, decimals)
            ));
        }
    }
    out
}

fn csv_cell(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn render_csv(f: &Fit, decimals: usize) -> String {
    let mut out = String::new();
    out.push_str("term,estimate,std_error\n");
    for t in &f.terms {
        out.push_str(&format!(
            "{},{},{}\n",
            csv_cell(&t.name),
            fmt_num(t.estimate, decimals),
            fmt_num(t.std_error, decimals)
        ));
    }
    out.push_str("\nstatistic,value\n");
    out.push_str(&format!("r_squared,{}\n", fmt_num(f.r_squared, decimals)));
    out.push_str(&format!(
        "adj_r_squared,{}\n",
        fmt_num(f.adj_r_squared, decimals)
    ));
    if let Some(r) = f.pearson_r {
        out.push_str(&format!("pearson_r,{}\n", fmt_num(r, decimals)));
    }
    out.push_str(&format!("rmse,{}\n", fmt_num(f.rmse, decimals)));
    out.push_str(&format!(
        "residual_std_error,{}\n",
        fmt_num(f.residual_std_error, decimals)
    ));
    out.push_str(&format!("df_residual,{}\n", f.df_residual));

    out.push_str(&format!(
        "\n{},{},fitted,residual\n",
        csv_cell(&f.x_label),
        csv_cell(&f.y_label)
    ));
    for pt in &f.points {
        out.push_str(&format!(
            "{},{},{},{}\n",
            fmt_num(pt.x, decimals),
            fmt_num(pt.y, decimals),
            fmt_num(pt.fitted, decimals),
            fmt_num(pt.residual, decimals)
        ));
    }

    if !f.predictions.is_empty() {
        out.push_str(&format!(
            "\n{},predicted_{}\n",
            csv_cell(&f.x_label),
            csv_cell(&f.y_label)
        ));
        for p in &f.predictions {
            out.push_str(&format!(
                "{},{}\n",
                fmt_num(p.x, decimals),
                fmt_num(p.y, decimals)
            ));
        }
    }
    out
}

/// Entry point shared by the chat block, the CLI and the page.
#[allow(clippy::too_many_arguments)]
pub fn run(
    data: &str,
    y_values: &str,
    degree: i64,
    header: &str,
    intercept: bool,
    predict_x: &str,
    decimals: i64,
    format: &str,
) -> Result<String, String> {
    let fmt = format.trim().to_ascii_lowercase();
    if !matches!(fmt.as_str(), "text" | "csv" | "json") {
        return Err(format!(
            "format must be 'text', 'csv' or 'json' (got '{format}')"
        ));
    }
    let f = fit(data, y_values, degree, header, intercept, predict_x, decimals)?;
    let d = decimals.clamp(0, 12) as usize;
    Ok(match fmt.as_str() {
        "json" => serde_json::to_string_pretty(&f).map_err(|e| e.to_string())?,
        "csv" => render_csv(&f, d),
        _ => render_text(&f, d),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fits_a_perfect_line() {
        let f = fit("1 2\n2 4\n3 6\n4 8", "", 1, "auto", true, "", 6).unwrap();
        assert_eq!(f.equation, "y = 2.000000·x + 0.000000");
        assert_eq!(f.terms[0].estimate, 0.0);
        assert_eq!(f.terms[1].estimate, 2.0);
        assert_eq!(f.r_squared, 1.0);
        assert_eq!(f.pearson_r, Some(1.0));
        assert_eq!(f.rmse, 0.0);
        assert_eq!(f.df_residual, 2);
        assert_eq!(f.n, 4);
    }

    #[test]
    fn fits_a_known_noisy_line() {
        // Textbook example: slope 0.6, intercept 2.2, R² = 0.6, RSS = 2.4.
        let f = fit("1,2\n2,4\n3,5\n4,4\n5,5", "", 1, "auto", true, "", 4).unwrap();
        assert_eq!(f.terms[0].estimate, 2.2);
        assert_eq!(f.terms[1].estimate, 0.6);
        assert_eq!(f.r_squared, 0.6);
        assert_eq!(f.adj_r_squared, 0.4667);
        assert_eq!(f.rss, 2.4);
        assert_eq!(f.rmse, round_to((2.4f64 / 5.0).sqrt(), 4));
    }

    #[test]
    fn fits_an_exact_quadratic() {
        // y = 1 + 2x + 3x²
        let f = fit("0,1\n1,6\n2,17\n3,34\n4,57", "", 2, "auto", true, "5", 6).unwrap();
        assert_eq!(f.terms[0].estimate, 1.0);
        assert_eq!(f.terms[1].estimate, 2.0);
        assert_eq!(f.terms[2].estimate, 3.0);
        assert_eq!(f.r_squared, 1.0);
        assert_eq!(f.pearson_r, None);
        assert_eq!(f.equation, "y = 3.000000·x² + 2.000000·x + 1.000000");
        assert_eq!(f.predictions[0].y, 86.0);
    }

    #[test]
    fn fits_a_cubic_and_recovers_negative_terms() {
        // y = x³ - 2x + 1 sampled on -3..=3
        let data: String = (-3..=3)
            .map(|x| {
                let xf = x as f64;
                format!("{x},{}\n", xf.powi(3) - 2.0 * xf + 1.0)
            })
            .collect();
        let f = fit(&data, "", 3, "auto", true, "", 4).unwrap();
        assert_eq!(f.terms[0].estimate, 1.0);
        assert_eq!(f.terms[1].estimate, -2.0);
        assert_eq!(f.terms[2].estimate, 0.0);
        assert_eq!(f.terms[3].estimate, 1.0);
        assert_eq!(f.equation, "y = 1.0000·x³ + 0.0000·x² - 2.0000·x + 1.0000");
    }

    #[test]
    fn honours_a_forced_zero_intercept() {
        // Through the origin: slope = Σxy / Σx² = 29.5 / 14.
        let f = fit("1,2\n2,4\n3,6.5", "", 1, "auto", false, "", 6).unwrap();
        assert_eq!(f.terms.len(), 1);
        assert_eq!(f.terms[0].power, 1);
        assert_eq!(f.df_residual, 2);
        assert_eq!(f.terms[0].estimate, round_to(29.5 / 14.0, 6));
        assert!(f.equation.starts_with("y = 2.107143·x"), "{}", f.equation);
    }

    #[test]
    fn reads_a_header_row_and_names_the_axes() {
        let f = fit("month,sales\n1,10\n2,12\n3,15\n4,19", "", 1, "auto", true, "", 3).unwrap();
        assert_eq!(f.x_label, "month");
        assert_eq!(f.y_label, "sales");
        assert_eq!(f.n, 4);
        assert!(f.equation.starts_with("sales = "), "{}", f.equation);
        assert_eq!(f.terms[1].name, "month");
    }

    #[test]
    fn header_no_rejects_a_text_first_row() {
        let err = fit("month,sales\n1,10\n2,12\n3,15", "", 1, "no", true, "", 3).unwrap_err();
        assert!(err.contains("not a number"), "{err}");
    }

    #[test]
    fn accepts_separate_x_and_y_lists() {
        let f = fit("1, 2, 3, 4", "2, 4, 6, 8", 1, "auto", true, "", 6).unwrap();
        assert_eq!(f.n, 4);
        assert_eq!(f.terms[1].estimate, 2.0);
    }

    #[test]
    fn separate_lists_take_their_own_headers() {
        let f = fit("time\n1\n2\n3\n4", "dist\n2\n4\n6\n8", 1, "auto", true, "", 6).unwrap();
        assert_eq!(f.x_label, "time");
        assert_eq!(f.y_label, "dist");
        assert_eq!(f.n, 4);
    }

    #[test]
    fn mismatched_list_lengths_are_rejected() {
        let err = fit("1,2,3", "1,2", 1, "auto", true, "", 6).unwrap_err();
        assert!(err.contains("same number of values"), "{err}");
    }

    #[test]
    fn rejects_rows_that_are_not_pairs() {
        let err = fit("1,2,3\n2,3,4\n3,4,5", "", 1, "auto", true, "", 6).unwrap_err();
        assert!(err.contains("expected 2 values"), "{err}");
    }

    #[test]
    fn rejects_too_few_points_for_the_degree() {
        let err = fit("1,1\n2,4\n3,9", "", 2, "auto", true, "", 6).unwrap_err();
        assert!(err.contains("not enough points"), "{err}");
    }

    #[test]
    fn rejects_too_few_distinct_x_values() {
        // Two distinct x values cannot support a quadratic.
        let err = fit("1,1\n1,2\n2,3\n2,4\n1,5", "", 2, "auto", true, "", 6).unwrap_err();
        assert!(err.contains("not enough distinct x values"), "{err}");
    }

    #[test]
    fn rejects_an_out_of_range_degree() {
        let err = fit("1,1\n2,2\n3,3", "", 0, "auto", true, "", 6).unwrap_err();
        assert!(err.contains("degree must be between 1 and 10"), "{err}");
        let err = fit("1,1\n2,2\n3,3", "", 11, "auto", true, "", 6).unwrap_err();
        assert!(err.contains("degree must be between 1 and 10"), "{err}");
    }

    #[test]
    fn rejects_empty_and_non_numeric_data() {
        assert!(fit("   \n\n", "", 1, "auto", true, "", 6).is_err());
        let err = fit("1,2\n2,abc\n3,4", "", 1, "auto", true, "", 6).unwrap_err();
        assert!(err.contains("'abc' is not a number"), "{err}");
    }

    #[test]
    fn rejects_a_bad_decimals_value() {
        let err = fit("1,1\n2,2\n3,3", "", 1, "auto", true, "", 13).unwrap_err();
        assert!(err.contains("decimals must be between 0 and 12"), "{err}");
    }

    #[test]
    fn rejects_a_bad_header_mode() {
        let err = fit("1,1\n2,2\n3,3", "", 1, "maybe", true, "", 6).unwrap_err();
        assert!(err.contains("header must be"), "{err}");
    }

    #[test]
    fn rejects_an_unknown_format() {
        let err = run("1,1\n2,2\n3,3", "", 1, "auto", true, "", 6, "xml").unwrap_err();
        assert!(err.contains("format must be"), "{err}");
    }

    #[test]
    fn rejects_a_bad_prediction_x() {
        let err = fit("1,2\n2,4\n3,6", "", 1, "auto", true, "4,later", 6).unwrap_err();
        assert!(err.contains("predict x"), "{err}");
    }

    #[test]
    fn tolerates_quoted_spreadsheet_cells_and_blank_lines() {
        let f =
            fit("\"1\",\"2\"\n\n\"2\",\"4\"\n\n\"3\",\"6\"\n", "", 1, "auto", true, "", 6).unwrap();
        assert_eq!(f.n, 3);
        assert_eq!(f.terms[1].estimate, 2.0);
    }

    #[test]
    fn decimals_control_the_rendering() {
        let out = run("1,2\n2,4\n3,5\n4,4\n5,5", "", 1, "auto", true, "", 2, "text").unwrap();
        assert!(out.starts_with("y = 0.60·x + 2.20\n"), "{out}");
    }

    #[test]
    fn text_output_has_every_section() {
        let out = run("1,2\n2,4\n3,5\n4,4\n5,5", "", 1, "auto", true, "6,7", 4, "text").unwrap();
        assert!(out.contains("Model"));
        assert!(out.contains("Coefficients"));
        assert!(out.contains("Residuals"));
        assert!(out.contains("Predictions"));
        assert!(out.contains("x = 6.0000  ->  y = 5.8000"), "{out}");
        assert!(out.contains("x = 7.0000  ->  y = 6.4000"), "{out}");
    }

    #[test]
    fn csv_output_has_the_three_blocks() {
        let out = run("1,2\n2,4\n3,6\n4,8", "", 1, "auto", true, "5", 2, "csv").unwrap();
        assert!(out.starts_with("term,estimate,std_error\n"), "{out}");
        assert!(out.contains("\nstatistic,value\n"), "{out}");
        assert!(out.contains("\nx,y,fitted,residual\n"), "{out}");
        assert!(out.contains("\nx,predicted_y\n5.00,10.00\n"), "{out}");
    }

    #[test]
    fn json_output_is_structured() {
        let out = run("1,2\n2,4\n3,6\n4,8", "", 1, "auto", true, "", 6, "json").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["degree"], 1);
        assert_eq!(v["n"], 4);
        assert_eq!(v["terms"][1]["estimate"], 2.0);
        assert_eq!(v["points"].as_array().unwrap().len(), 4);
        assert_eq!(v["r_squared"], 1.0);
        assert_eq!(v["pearson_r"], 1.0);
    }

    #[test]
    fn high_degree_fit_stays_finite() {
        // 13 points on y = x fitted with a degree-10 polynomial: the scaled QR
        // path must still return a finite fit that reproduces the data.
        let data: String = (-6..=6).map(|x| format!("{x},{x}\n")).collect();
        let f = fit(&data, "", 10, "auto", true, "", 6).unwrap();
        assert!(f.r_squared > 0.999, "R² was {}", f.r_squared);
        assert!(f.terms.iter().all(|t| t.estimate.is_finite()));
    }

    #[test]
    fn rejects_too_many_points() {
        let data: String = (0..MAX_POINTS + 1).map(|i| format!("{i},{i}\n")).collect();
        let err = fit(&data, "", 1, "auto", true, "", 6).unwrap_err();
        assert!(err.contains("too many points"), "{err}");
    }
}
