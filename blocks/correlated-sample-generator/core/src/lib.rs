//! correlated-sample-generator core — pure compute, shared by the chat skill
//! block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Draws samples from a multivariate normal distribution with a supplied mean
//! vector and covariance (or correlation) matrix. Independent standard normals
//! `z` are turned into correlated draws by `y = mean + A z`, where `A` is a
//! factor of the target matrix satisfying `A Aᵀ = Σ`:
//!
//! * `method = "cholesky"` — the lower-triangular Cholesky factor. Fast, and
//!   the classic recipe, but it needs Σ to be strictly positive definite.
//! * `method = "eigen"` — the symmetric square root `V √Λ Vᵀ` from a Jacobi
//!   eigendecomposition. Slower, but it also handles positive-SEMI-definite and
//!   numerically singular matrices (perfectly correlated variables, ρ = ±1).
//!
//! With `empirical = true` the draws are whitened and re-coloured so the
//! SAMPLE mean and SAMPLE covariance match the targets exactly, rather than
//! only in expectation (the behaviour of R's `MASS::mvrnorm(empirical = TRUE)`).
//!
//! Randomness is a seeded, hand-rolled SplitMix64 PRNG feeding Box–Muller — the
//! same stream on every backend, with no `getrandom`, so chat, CLI, page and
//! tests all agree for a given `seed`.

use serde_json::{json, Map, Value};

/// Hard cap on the number of draws so a runaway request can't hang a browser tab.
pub const MAX_SAMPLES: usize = 100_000;
/// Hard cap on the number of variables (matrix side length).
pub const MAX_DIM: usize = 50;
/// Hard cap on total emitted numbers (`samples × dimensions`).
pub const MAX_CELLS: usize = 200_000;
/// Hard cap on rounding precision.
pub const MAX_DECIMALS: usize = 12;

/// SplitMix64 — a tiny deterministic PRNG, identical on every target.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        // Offset the state so seed = 0 isn't a degenerate stream.
        Rng(seed ^ 0x9E37_79B9_7F4A_7C15)
    }
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    /// Uniform double in `(0, 1)` — never exactly 0, so `ln()` stays finite.
    fn unit_pos(&mut self) -> f64 {
        ((self.next_u64() >> 11) + 1) as f64 / 9_007_199_254_740_993.0 // 2^53 + 1
    }
    /// Standard normal via Box–Muller (two uniforms per draw, second discarded
    /// so the stream position depends only on the draw count).
    fn normal(&mut self) -> f64 {
        let u1 = self.unit_pos();
        let u2 = self.unit_pos();
        (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos()
    }
}

// ---------------------------------------------------------------- parsing ---

/// Expand a structured shorthand into a full correlation matrix, or return
/// `None` when `t` isn't one. Three structures cover the usual repeated-measures
/// and panel designs without typing out every entry:
///
/// * `iid(d)` — the `d`x`d` identity (uncorrelated variables);
/// * `cs(d, rho)` — compound symmetry / exchangeable: `rho` between every pair;
/// * `ar1(d, rho)` — first-order autoregressive: `rho^|i-j|`, so correlation
///   decays with the distance between variables.
fn parse_structured(t: &str, what: &str) -> Option<Result<Vec<Vec<f64>>, String>> {
    let open = t.find('(')?;
    let name = t[..open].trim().to_ascii_lowercase();
    let kind = match name.as_str() {
        "iid" | "identity" | "independent" => "iid",
        "cs" | "exchangeable" | "compound" => "cs",
        "ar1" | "ar" => "ar1",
        _ => return None,
    };
    Some((|| {
        let inner = t
            .strip_suffix(')')
            .map(|s| &s[open + 1..])
            .ok_or_else(|| format!("{what}: {kind}(...) is missing its closing parenthesis"))?;
        let args: Vec<&str> = inner
            .split(',')
            .map(str::trim)
            .filter(|a| !a.is_empty())
            .collect();
        let want = if kind == "iid" { 1 } else { 2 };
        if args.len() != want {
            return Err(format!(
                "{what}: {} takes {want} value(s) but got {} — write {}",
                kind,
                args.len(),
                if kind == "iid" {
                    "iid(4) for 4 uncorrelated variables"
                } else if kind == "cs" {
                    "cs(4, 0.3) for 4 variables correlated 0.3 with each other"
                } else {
                    "ar1(4, 0.7) for 4 variables with correlation 0.7^|i-j|"
                }
            ));
        }
        let d: usize = args[0].parse().map_err(|_| {
            format!(
                "{what}: {kind} needs a whole number of variables, got '{}'",
                args[0]
            )
        })?;
        if d == 0 || d > MAX_DIM {
            return Err(format!(
                "{what}: {kind} was asked for {d} variable(s); it must be between 1 and {MAX_DIM}"
            ));
        }
        let rho = if kind == "iid" {
            0.0
        } else {
            let r: f64 = args[1]
                .parse()
                .map_err(|_| format!("{what}: {kind} needs a numeric correlation, got '{}'", args[1]))?;
            if !r.is_finite() || r.abs() > 1.0 {
                return Err(format!(
                    "{what}: {kind} correlation is {}, outside the valid range -1 to 1",
                    args[1]
                ));
            }
            r
        };
        Ok((0..d)
            .map(|i| {
                (0..d)
                    .map(|j| match kind {
                        _ if i == j => 1.0,
                        "cs" => rho,
                        "ar1" => rho.powi((i as i32 - j as i32).abs()),
                        _ => 0.0,
                    })
                    .collect()
            })
            .collect())
    })())
}

/// Parse a square matrix. Accepts JSON (`[[1,0.8],[0.8,1]]`), one row per line,
/// MATLAB-ish `;`-separated rows (`1, 0.8; 0.8, 1`), or a structured shorthand
/// (`iid(3)`, `cs(4, 0.3)`, `ar1(5, 0.7)`); entries may be split by commas, tabs
/// or spaces. Brackets and parentheses are ignored.
fn parse_matrix(s: &str, what: &str) -> Result<Vec<Vec<f64>>, String> {
    let t = s.trim();
    if t.is_empty() {
        return Err(format!(
            "{what} is empty — supply a square matrix, e.g. \"1, 0.8; 0.8, 1\", one row per line, JSON [[1,0.8],[0.8,1]], or a shorthand such as ar1(4, 0.7)"
        ));
    }
    if let Some(structured) = parse_structured(t, what) {
        return structured;
    }
    if t.starts_with('[') {
        if let Ok(rows) = serde_json::from_str::<Vec<Vec<f64>>>(t) {
            if !rows.is_empty() && rows.iter().all(|r| !r.is_empty()) {
                return Ok(rows);
            }
        }
    }
    // Treat `],` as a row break so a slightly malformed JSON-ish paste still works.
    let normalized = t.replace("],", ";");
    let cleaned: String = normalized
        .chars()
        .map(|c| match c {
            '[' | ']' | '(' | ')' | '\r' => ' ',
            other => other,
        })
        .collect();
    let mut rows: Vec<Vec<f64>> = Vec::new();
    for raw in cleaned.split(['\n', ';']) {
        if raw.trim().is_empty() {
            continue;
        }
        let mut vals = Vec::new();
        for tok in raw.split(|c: char| c == ',' || c.is_whitespace()) {
            if tok.is_empty() {
                continue;
            }
            let v: f64 = tok.parse().map_err(|_| {
                format!(
                    "{what}: '{tok}' on row {} is not a number — expected decimal values like 1, 0.8 or -2.5e-3",
                    rows.len() + 1
                )
            })?;
            if !v.is_finite() {
                return Err(format!(
                    "{what}: '{tok}' on row {} is not a finite number",
                    rows.len() + 1
                ));
            }
            vals.push(v);
        }
        if !vals.is_empty() {
            rows.push(vals);
        }
    }
    if rows.is_empty() {
        return Err(format!("{what} contains no numbers"));
    }
    Ok(rows)
}

/// Parse a vector of numbers. Accepts `1, 2`, `[1, 2]`, `1 2` or one per line.
fn parse_vector(s: &str, what: &str) -> Result<Vec<f64>, String> {
    let cleaned: String = s
        .chars()
        .map(|c| match c {
            '[' | ']' | '(' | ')' | '\r' | '\n' | ';' => ' ',
            other => other,
        })
        .collect();
    let mut out = Vec::new();
    for tok in cleaned.split(|c: char| c == ',' || c.is_whitespace()) {
        if tok.is_empty() {
            continue;
        }
        let v: f64 = tok.parse().map_err(|_| {
            format!("{what}: '{tok}' is not a number — expected decimal values like 0, -1.5, 2e3")
        })?;
        if !v.is_finite() {
            return Err(format!("{what}: '{tok}' is not a finite number"));
        }
        out.push(v);
    }
    Ok(out)
}

/// Parse comma/newline-separated column labels; empty entries are dropped.
fn parse_labels(s: &str) -> Vec<String> {
    s.split([',', '\n', ';'])
        .map(|t| t.trim())
        .filter(|t| !t.is_empty())
        .map(|t| t.to_string())
        .collect()
}

// --------------------------------------------------------- linear algebra ---

/// Largest absolute diagonal entry, floored at 1.0 — the scale tolerances ride on.
fn diag_scale(m: &[Vec<f64>]) -> f64 {
    m.iter()
        .enumerate()
        .map(|(i, row)| row[i].abs())
        .fold(1.0_f64, f64::max)
}

/// Lower-triangular Cholesky factor `L` with `L Lᵀ = m`. Errors when `m` is not
/// strictly positive definite.
fn cholesky(m: &[Vec<f64>], tol: f64, labels: &[String], what: &str) -> Result<Vec<Vec<f64>>, String> {
    let d = m.len();
    let scale = diag_scale(m);
    let mut l = vec![vec![0.0_f64; d]; d];
    for i in 0..d {
        for j in 0..=i {
            let mut sum = m[i][j];
            for k in 0..j {
                sum -= l[i][k] * l[j][k];
            }
            if i == j {
                if sum <= tol * scale {
                    return Err(format!(
                        "{what} is not positive definite: the Cholesky pivot for {} is {} (it must be greater than 0). Redundant or perfectly correlated variables make the matrix singular — set method to 'eigen', which also handles positive-semi-definite and numerically singular matrices.",
                        labels.get(i).cloned().unwrap_or_else(|| format!("variable {}", i + 1)),
                        fmt_num(sum, 6)
                    ));
                }
                l[i][j] = sum.sqrt();
            } else {
                l[i][j] = sum / l[j][j];
            }
        }
    }
    Ok(l)
}

/// Cyclic Jacobi eigendecomposition of a symmetric matrix.
/// Returns `(eigenvalues, eigenvectors)` with eigenvector `k` in COLUMN `k`.
fn jacobi_eigen(m: &[Vec<f64>]) -> (Vec<f64>, Vec<Vec<f64>>) {
    let d = m.len();
    let mut a: Vec<Vec<f64>> = m.to_vec();
    let mut v = vec![vec![0.0_f64; d]; d];
    for (i, row) in v.iter_mut().enumerate() {
        row[i] = 1.0;
    }
    let norm: f64 = a.iter().flatten().map(|x| x * x).sum();
    let threshold = 1e-30_f64.max(norm * 1e-30);
    for _sweep in 0..100 {
        let mut off = 0.0;
        for i in 0..d {
            for j in (i + 1)..d {
                off += a[i][j] * a[i][j];
            }
        }
        if off <= threshold {
            break;
        }
        for p in 0..d {
            for q in (p + 1)..d {
                if a[p][q] == 0.0 {
                    continue;
                }
                let theta = (a[q][q] - a[p][p]) / (2.0 * a[p][q]);
                let t = theta.signum() / (theta.abs() + (theta * theta + 1.0).sqrt());
                let c = 1.0 / (t * t + 1.0).sqrt();
                let s = t * c;
                for k in 0..d {
                    let (akp, akq) = (a[k][p], a[k][q]);
                    a[k][p] = c * akp - s * akq;
                    a[k][q] = s * akp + c * akq;
                }
                for k in 0..d {
                    let (apk, aqk) = (a[p][k], a[q][k]);
                    a[p][k] = c * apk - s * aqk;
                    a[q][k] = s * apk + c * aqk;
                }
                for k in 0..d {
                    let (vkp, vkq) = (v[k][p], v[k][q]);
                    v[k][p] = c * vkp - s * vkq;
                    v[k][q] = s * vkp + c * vkq;
                }
            }
        }
    }
    let eig = (0..d).map(|i| a[i][i]).collect();
    (eig, v)
}

/// Symmetric square root `A = V √Λ Vᵀ` (so `A Aᵀ = m`). Eigenvalues slightly
/// below zero are clamped; genuinely negative ones are an error.
fn sym_sqrt(m: &[Vec<f64>], tol: f64, what: &str) -> Result<Vec<Vec<f64>>, String> {
    let d = m.len();
    let scale = diag_scale(m);
    let (eig, v) = jacobi_eigen(m);
    let mut root = vec![0.0_f64; d];
    for (k, &lambda) in eig.iter().enumerate() {
        if lambda < -tol * scale {
            return Err(format!(
                "{what} is not positive semi-definite: it has eigenvalue {} , which is below the allowed tolerance of -{}. A valid covariance matrix has no negative eigenvalues — check the off-diagonal entries, or raise tol if this is only rounding noise.",
                fmt_num(lambda, 6),
                fmt_num(tol * scale, 10)
            ));
        }
        root[k] = lambda.max(0.0).sqrt();
    }
    let mut a = vec![vec![0.0_f64; d]; d];
    for i in 0..d {
        for j in 0..d {
            let mut acc = 0.0;
            for k in 0..d {
                acc += v[i][k] * root[k] * v[j][k];
            }
            a[i][j] = acc;
        }
    }
    Ok(a)
}

/// Solve `L x = b` for lower-triangular `L` (forward substitution).
fn forward_solve(l: &[Vec<f64>], b: &[f64]) -> Vec<f64> {
    let d = b.len();
    let mut x = vec![0.0_f64; d];
    for i in 0..d {
        let mut sum = b[i];
        for k in 0..i {
            sum -= l[i][k] * x[k];
        }
        x[i] = if l[i][i] == 0.0 { 0.0 } else { sum / l[i][i] };
    }
    x
}

/// Sample mean of each column.
fn column_means(rows: &[Vec<f64>]) -> Vec<f64> {
    let d = rows.first().map(|r| r.len()).unwrap_or(0);
    let n = rows.len() as f64;
    let mut mu = vec![0.0_f64; d];
    for row in rows {
        for (j, v) in row.iter().enumerate() {
            mu[j] += v;
        }
    }
    for m in mu.iter_mut() {
        *m /= n;
    }
    mu
}

/// Unbiased (ddof = 1) sample covariance matrix — matches R's `var()` and
/// NumPy's `np.cov()` default. Needs at least two rows.
fn sample_covariance(rows: &[Vec<f64>], means: &[f64]) -> Vec<Vec<f64>> {
    let d = means.len();
    let n = rows.len();
    let mut cov = vec![vec![0.0_f64; d]; d];
    if n < 2 {
        return cov;
    }
    for row in rows {
        for i in 0..d {
            let di = row[i] - means[i];
            for j in 0..d {
                cov[i][j] += di * (row[j] - means[j]);
            }
        }
    }
    let denom = (n - 1) as f64;
    for row in cov.iter_mut() {
        for v in row.iter_mut() {
            *v /= denom;
        }
    }
    cov
}

/// Correlation matrix derived from a covariance matrix (0 where a variance is 0).
fn to_correlation(cov: &[Vec<f64>]) -> Vec<Vec<f64>> {
    let d = cov.len();
    let sd: Vec<f64> = (0..d).map(|i| cov[i][i].max(0.0).sqrt()).collect();
    let mut corr = vec![vec![0.0_f64; d]; d];
    for i in 0..d {
        for j in 0..d {
            corr[i][j] = if sd[i] > 0.0 && sd[j] > 0.0 {
                cov[i][j] / (sd[i] * sd[j])
            } else {
                0.0
            };
        }
    }
    corr
}

/// Largest absolute difference between two equally shaped matrices.
fn max_abs_diff(a: &[Vec<f64>], b: &[Vec<f64>]) -> f64 {
    a.iter()
        .zip(b)
        .flat_map(|(ra, rb)| ra.iter().zip(rb).map(|(x, y)| (x - y).abs()))
        .fold(0.0_f64, f64::max)
}

// ------------------------------------------------------------ formatting ---

/// Fixed-decimal rendering that never prints a signed zero.
fn fmt_num(v: f64, decimals: usize) -> String {
    let s = format!("{:.*}", decimals, v);
    if let Some(rest) = s.strip_prefix('-') {
        if rest.chars().all(|c| c == '0' || c == '.') {
            return rest.to_string();
        }
    }
    s
}

/// Round to `decimals` places for JSON output, collapsing `-0.0` to `0.0`.
fn round_num(v: f64, decimals: usize) -> f64 {
    let f = 10f64.powi(decimals as i32);
    if !f.is_finite() || !(v * f).is_finite() {
        return v;
    }
    let r = (v * f).round() / f;
    if r == 0.0 {
        0.0
    } else {
        r
    }
}

/// Render a labelled matrix as an aligned text table.
fn matrix_table(m: &[Vec<f64>], labels: &[String], decimals: usize) -> String {
    let d = m.len();
    let cells: Vec<Vec<String>> = m
        .iter()
        .map(|row| row.iter().map(|v| fmt_num(*v, decimals)).collect())
        .collect();
    let label_w = labels.iter().map(|l| l.chars().count()).max().unwrap_or(1);
    let mut col_w = vec![0usize; d];
    for (j, w) in col_w.iter_mut().enumerate() {
        *w = labels[j].chars().count();
        for row in &cells {
            *w = (*w).max(row[j].chars().count());
        }
    }
    let mut out = String::new();
    out.push_str(&" ".repeat(label_w + 2));
    for (j, w) in col_w.iter().enumerate() {
        out.push_str(&format!("{:>width$}  ", labels[j], width = w));
    }
    out = out.trim_end().to_string();
    out.push('\n');
    for i in 0..d {
        out.push_str(&format!("{:<width$}  ", labels[i], width = label_w));
        for (j, w) in col_w.iter().enumerate() {
            out.push_str(&format!("{:>width$}  ", cells[i][j], width = w));
        }
        out = out.trim_end().to_string();
        out.push('\n');
    }
    out
}

/// `[[…]]` as a JSON value with each entry rounded.
fn matrix_json(m: &[Vec<f64>], decimals: usize) -> Value {
    Value::Array(
        m.iter()
            .map(|row| {
                Value::Array(
                    row.iter()
                        .map(|v| json!(round_num(*v, decimals)))
                        .collect(),
                )
            })
            .collect(),
    )
}

fn vector_json(v: &[f64], decimals: usize) -> Value {
    Value::Array(v.iter().map(|x| json!(round_num(*x, decimals))).collect())
}

// ---------------------------------------------------------------- driver ---

/// Generate correlated multivariate-normal samples and render them.
///
/// * `covariance` — the square target matrix (covariance or correlation, per `matrix_kind`).
/// * `mean` — per-variable means; empty = all zeros.
/// * `samples` — number of draws.
/// * `matrix_kind` — `covariance` | `correlation`.
/// * `sd` — per-variable standard deviations, used only with `matrix_kind = correlation`; empty = all 1.
/// * `method` — `cholesky` | `eigen`.
/// * `seed` — PRNG seed; the same seed always yields the same draws.
/// * `empirical` — force the SAMPLE mean/covariance to equal the targets exactly.
/// * `tol` — definiteness/symmetry tolerance, relative to the matrix scale.
/// * `output` — `csv` | `tsv` | `json` | `stats`.
/// * `decimals` — rounding precision, 0-12.
/// * `labels` — column names; empty = `X1…Xd`.
/// * `header` — emit the column-name row in `csv`/`tsv`.
#[allow(clippy::too_many_arguments)]
pub fn generate(
    covariance: &str,
    mean: &str,
    samples: usize,
    matrix_kind: &str,
    sd: &str,
    method: &str,
    seed: u64,
    empirical: bool,
    tol: f64,
    output: &str,
    decimals: usize,
    labels: &str,
    header: bool,
) -> Result<String, String> {
    // ---- enum-ish params -------------------------------------------------
    let kind = match matrix_kind.trim().to_ascii_lowercase().as_str() {
        "" | "covariance" | "cov" => "covariance",
        "correlation" | "corr" => "correlation",
        other => {
            return Err(format!(
                "matrix_kind must be 'covariance' or 'correlation' (got '{other}')"
            ))
        }
    };
    let meth = match method.trim().to_ascii_lowercase().as_str() {
        "" | "cholesky" => "cholesky",
        "eigen" | "eigh" => "eigen",
        other => {
            return Err(format!(
                "method must be 'cholesky' or 'eigen' (got '{other}')"
            ))
        }
    };
    let out_fmt = match output.trim().to_ascii_lowercase().as_str() {
        "" | "csv" => "csv",
        "tsv" => "tsv",
        "json" => "json",
        "stats" => "stats",
        other => {
            return Err(format!(
                "output must be 'csv', 'tsv', 'json' or 'stats' (got '{other}')"
            ))
        }
    };
    if decimals > MAX_DECIMALS {
        return Err(format!(
            "decimals must be between 0 and {MAX_DECIMALS} (got {decimals})"
        ));
    }
    if !tol.is_finite() || tol < 0.0 {
        return Err(format!(
            "tol must be a finite number greater than or equal to 0 (got {tol})"
        ));
    }

    // ---- matrix ----------------------------------------------------------
    let matrix_name = if kind == "correlation" {
        "correlation matrix"
    } else {
        "covariance matrix"
    };
    let raw = parse_matrix(covariance, matrix_name)?;
    let d = raw.len();
    if d > MAX_DIM {
        return Err(format!(
            "{matrix_name} has {d} variables, above the maximum of {MAX_DIM}"
        ));
    }
    for (i, row) in raw.iter().enumerate() {
        if row.len() != d {
            return Err(format!(
                "{matrix_name} must be square: it has {d} rows but row {} has {} entries",
                i + 1,
                row.len()
            ));
        }
    }

    // ---- labels ----------------------------------------------------------
    let names = parse_labels(labels);
    let names: Vec<String> = if names.is_empty() {
        (1..=d).map(|i| format!("X{i}")).collect()
    } else if names.len() == d {
        names
    } else {
        return Err(format!(
            "labels lists {} name(s) but the matrix has {d} variable(s)",
            names.len()
        ));
    };

    // ---- symmetry --------------------------------------------------------
    let scale = diag_scale(&raw);
    for i in 0..d {
        for j in (i + 1)..d {
            let (a, b) = (raw[i][j], raw[j][i]);
            if (a - b).abs() > tol.max(1e-9) * (scale + a.abs().max(b.abs())) {
                return Err(format!(
                    "{matrix_name} must be symmetric: entry ({},{}) is {} but ({},{}) is {}",
                    i + 1,
                    j + 1,
                    fmt_num(a, 6),
                    j + 1,
                    i + 1,
                    fmt_num(b, 6)
                ));
            }
        }
    }

    // ---- covariance vs correlation --------------------------------------
    let sigma: Vec<Vec<f64>> = if kind == "correlation" {
        for i in 0..d {
            if (raw[i][i] - 1.0).abs() > 1e-6 {
                return Err(format!(
                    "a correlation matrix must have 1 on the diagonal: {} is {}. Set matrix_kind to 'covariance' if these are variances.",
                    names[i],
                    fmt_num(raw[i][i], 6)
                ));
            }
            for j in 0..d {
                if raw[i][j].abs() > 1.0 + 1e-9 {
                    return Err(format!(
                        "correlation ({},{}) is {}, outside the valid range -1 to 1",
                        i + 1,
                        j + 1,
                        fmt_num(raw[i][j], 6)
                    ));
                }
            }
        }
        let sds = parse_vector(sd, "sd")?;
        let sds: Vec<f64> = if sds.is_empty() {
            vec![1.0; d]
        } else if sds.len() == d {
            sds
        } else {
            return Err(format!(
                "sd lists {} value(s) but the matrix has {d} variable(s)",
                sds.len()
            ));
        };
        if let Some(i) = sds.iter().position(|s| *s < 0.0) {
            return Err(format!(
                "sd for {} is {}; a standard deviation cannot be negative",
                names[i],
                fmt_num(sds[i], 6)
            ));
        }
        (0..d)
            .map(|i| (0..d).map(|j| raw[i][j] * sds[i] * sds[j]).collect())
            .collect()
    } else {
        for i in 0..d {
            if raw[i][i] < -tol.max(1e-12) * scale {
                return Err(format!(
                    "the variance for {} is {}; a covariance matrix cannot have a negative diagonal entry",
                    names[i],
                    fmt_num(raw[i][i], 6)
                ));
            }
        }
        if !sd.trim().is_empty() {
            return Err(
                "sd only applies when matrix_kind is 'correlation' — with 'covariance' the variances already come from the diagonal".to_string(),
            );
        }
        raw.clone()
    };

    // ---- mean vector -----------------------------------------------------
    let mu = parse_vector(mean, "mean")?;
    let mu: Vec<f64> = if mu.is_empty() {
        vec![0.0; d]
    } else if mu.len() == d {
        mu
    } else {
        return Err(format!(
            "mean lists {} value(s) but the matrix has {d} variable(s)",
            mu.len()
        ));
    };

    // ---- sizes -----------------------------------------------------------
    if samples == 0 {
        return Err("samples must be at least 1".to_string());
    }
    if samples > MAX_SAMPLES {
        return Err(format!(
            "samples is {samples}, above the maximum of {MAX_SAMPLES}"
        ));
    }
    if samples * d > MAX_CELLS {
        return Err(format!(
            "{samples} draws x {d} variables is {} numbers, above the maximum of {MAX_CELLS}. Lower samples to {} or fewer.",
            samples * d,
            MAX_CELLS / d
        ));
    }
    if empirical && samples <= d {
        return Err(format!(
            "empirical needs more draws than variables: samples is {samples} but the matrix has {d} variable(s). Use at least {} draws, or set empirical to false.",
            d + 1
        ));
    }

    // ---- factor ----------------------------------------------------------
    let factor = if meth == "cholesky" {
        cholesky(&sigma, tol, &names, matrix_name)?
    } else {
        sym_sqrt(&sigma, tol, matrix_name)?
    };

    // ---- draw ------------------------------------------------------------
    let mut rng = Rng::new(seed);
    let mut z: Vec<Vec<f64>> = (0..samples)
        .map(|_| (0..d).map(|_| rng.normal()).collect())
        .collect();

    if empirical {
        // Centre, then whiten with the sample covariance's Cholesky factor so
        // the standardised draws have sample mean 0 and sample covariance I
        // exactly; colouring with `factor` then reproduces the targets exactly.
        let zm = column_means(&z);
        for row in z.iter_mut() {
            for (j, v) in row.iter_mut().enumerate() {
                *v -= zm[j];
            }
        }
        let zcov = sample_covariance(&z, &vec![0.0; d]);
        let lz = cholesky(&zcov, 1e-12, &names, "the drawn sample's covariance")
            .map_err(|_| {
                "empirical could not be applied: the drawn sample is degenerate for this seed. Increase samples or change seed.".to_string()
            })?;
        for row in z.iter_mut() {
            *row = forward_solve(&lz, row);
        }
    }

    let rows: Vec<Vec<f64>> = z
        .iter()
        .map(|zi| {
            (0..d)
                .map(|i| mu[i] + (0..d).map(|k| factor[i][k] * zi[k]).sum::<f64>())
                .collect()
        })
        .collect();

    // ---- render ----------------------------------------------------------
    match out_fmt {
        "csv" | "tsv" => {
            let sep = if out_fmt == "tsv" { '\t' } else { ',' };
            let mut out = String::new();
            if header {
                out.push_str(&names.join(&sep.to_string()));
                out.push('\n');
            }
            for row in &rows {
                let cells: Vec<String> = row.iter().map(|v| fmt_num(*v, decimals)).collect();
                out.push_str(&cells.join(&sep.to_string()));
                out.push('\n');
            }
            Ok(out)
        }
        "json" => {
            let means = column_means(&rows);
            let cov = sample_covariance(&rows, &means);
            let mut achieved = Map::new();
            achieved.insert("mean".into(), vector_json(&means, decimals));
            if samples >= 2 {
                achieved.insert("covariance".into(), matrix_json(&cov, decimals));
                achieved.insert(
                    "correlation".into(),
                    matrix_json(&to_correlation(&cov), decimals),
                );
            } else {
                achieved.insert("covariance".into(), Value::Null);
                achieved.insert("correlation".into(), Value::Null);
            }
            let doc = json!({
                "count": samples,
                "dimensions": d,
                "labels": names,
                "method": meth,
                "seed": seed,
                "empirical": empirical,
                "target": {
                    "mean": vector_json(&mu, decimals),
                    "covariance": matrix_json(&sigma, decimals),
                },
                "achieved": Value::Object(achieved),
                "samples": Value::Array(
                    rows.iter().map(|r| vector_json(r, decimals)).collect(),
                ),
            });
            serde_json::to_string_pretty(&doc).map_err(|e| e.to_string())
        }
        _ => {
            let means = column_means(&rows);
            let cov = sample_covariance(&rows, &means);
            let mut out = String::new();
            out.push_str(&format!(
                "{samples} draw(s) of {d} variable(s) — method {meth}, seed {seed}, empirical {empirical}\nVariables: {}\n\n",
                names.join(", ")
            ));
            out.push_str(&format!(
                "Target mean:    {}\nSample mean:    {}\n",
                mu.iter()
                    .map(|v| fmt_num(*v, decimals))
                    .collect::<Vec<_>>()
                    .join(", "),
                means
                    .iter()
                    .map(|v| fmt_num(*v, decimals))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
            let mean_err = mu
                .iter()
                .zip(&means)
                .map(|(a, b)| (a - b).abs())
                .fold(0.0_f64, f64::max);
            out.push_str(&format!(
                "Largest mean difference: {}\n\nTarget covariance:\n{}",
                fmt_num(mean_err, decimals.max(4)),
                matrix_table(&sigma, &names, decimals)
            ));
            if samples >= 2 {
                out.push_str(&format!(
                    "\nSample covariance (ddof=1):\n{}\nLargest covariance difference: {}\n\nSample correlation:\n{}",
                    matrix_table(&cov, &names, decimals),
                    fmt_num(max_abs_diff(&sigma, &cov), decimals.max(4)),
                    matrix_table(&to_correlation(&cov), &names, decimals)
                ));
            } else {
                out.push_str("\nSample covariance needs at least 2 draws.\n");
            }
            Ok(out)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stats(cov: &str, n: usize) -> String {
        generate(cov, "", n, "covariance", "", "cholesky", 7, false, 1e-8, "stats", 4, "", true)
            .unwrap()
    }

    #[test]
    fn csv_output_is_deterministic_for_a_seed() {
        let a = generate(
            "1, 0; 0, 1", "", 3, "covariance", "", "cholesky", 42, false, 1e-8, "csv", 4, "", true,
        )
        .unwrap();
        let b = generate(
            "1, 0; 0, 1", "", 3, "covariance", "", "cholesky", 42, false, 1e-8, "csv", 4, "", true,
        )
        .unwrap();
        assert_eq!(a, b);
        assert_eq!(a.lines().next().unwrap(), "X1,X2");
        assert_eq!(a.lines().count(), 4);
        // A different seed must give different draws.
        let c = generate(
            "1, 0; 0, 1", "", 3, "covariance", "", "cholesky", 43, false, 1e-8, "csv", 4, "", true,
        )
        .unwrap();
        assert_ne!(a, c);
    }

    #[test]
    fn header_can_be_turned_off_and_tsv_uses_tabs() {
        let out = generate(
            "1, 0; 0, 1", "", 2, "covariance", "", "cholesky", 1, false, 1e-8, "tsv", 2, "a,b",
            false,
        )
        .unwrap();
        assert!(!out.contains("a\tb"), "header row must be omitted: {out}");
        assert_eq!(out.lines().count(), 2);
        assert!(out.lines().all(|l| l.split('\t').count() == 2), "{out}");
    }

    #[test]
    fn samples_reproduce_the_requested_correlation() {
        let out = stats("1, 0.8; 0.8, 1", 20_000);
        // Parse the sample correlation table's off-diagonal entry.
        let corr_block = out.split("Sample correlation:").nth(1).unwrap();
        let row_x1 = corr_block
            .lines()
            .find(|l| l.starts_with("X1"))
            .unwrap();
        let rho: f64 = row_x1.split_whitespace().nth(2).unwrap().parse().unwrap();
        assert!((rho - 0.8).abs() < 0.02, "correlation drifted: {rho}");
    }

    #[test]
    fn mean_and_standard_deviations_are_honoured() {
        let out = generate(
            "1, 0.5; 0.5, 1",
            "10, -4",
            20_000,
            "correlation",
            "2, 3",
            "eigen",
            5,
            false,
            1e-8,
            "json",
            4,
            "",
            true,
        )
        .unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        // Correlation 0.5 with sd 2 and 3 => covariance 3.
        assert_eq!(v["target"]["covariance"][0][1].as_f64().unwrap(), 3.0);
        let m0 = v["achieved"]["mean"][0].as_f64().unwrap();
        let m1 = v["achieved"]["mean"][1].as_f64().unwrap();
        assert!((m0 - 10.0).abs() < 0.1, "mean X1 drifted: {m0}");
        assert!((m1 + 4.0).abs() < 0.1, "mean X2 drifted: {m1}");
        let c00 = v["achieved"]["covariance"][0][0].as_f64().unwrap();
        assert!((c00 - 4.0).abs() < 0.2, "variance X1 drifted: {c00}");
    }

    #[test]
    fn empirical_matches_the_targets_exactly() {
        let out = generate(
            "4, 2; 2, 3",
            "1, -1",
            200,
            "covariance",
            "",
            "cholesky",
            9,
            true,
            1e-8,
            "json",
            6,
            "",
            true,
        )
        .unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["achieved"]["mean"][0].as_f64().unwrap(), 1.0);
        assert_eq!(v["achieved"]["mean"][1].as_f64().unwrap(), -1.0);
        assert_eq!(v["achieved"]["covariance"][0][0].as_f64().unwrap(), 4.0);
        assert_eq!(v["achieved"]["covariance"][0][1].as_f64().unwrap(), 2.0);
        assert_eq!(v["achieved"]["covariance"][1][1].as_f64().unwrap(), 3.0);
    }

    #[test]
    fn json_and_matlab_and_newline_matrix_forms_agree() {
        let a = generate(
            "[[4,2],[2,3]]", "", 5, "covariance", "", "cholesky", 3, false, 1e-8, "csv", 4, "",
            true,
        )
        .unwrap();
        let b = generate(
            "4, 2; 2, 3", "", 5, "covariance", "", "cholesky", 3, false, 1e-8, "csv", 4, "", true,
        )
        .unwrap();
        let c = generate(
            "4 2\n2 3", "", 5, "covariance", "", "cholesky", 3, false, 1e-8, "csv", 4, "", true,
        )
        .unwrap();
        assert_eq!(a, b);
        assert_eq!(b, c);
    }

    #[test]
    fn singular_matrix_fails_under_cholesky_but_works_under_eigen() {
        let err = generate(
            "1, 1; 1, 1", "", 5, "covariance", "", "cholesky", 1, false, 1e-8, "csv", 4, "", true,
        )
        .unwrap_err();
        assert!(err.contains("not positive definite"), "{err}");
        assert!(err.contains("eigen"), "error must point at the fix: {err}");
        let ok = generate(
            "1, 1; 1, 1", "", 5, "covariance", "", "eigen", 1, false, 1e-8, "csv", 4, "", true,
        )
        .unwrap();
        // Perfectly correlated: both columns are identical in every row.
        for line in ok.lines().skip(1) {
            let (a, b) = line.split_once(',').unwrap();
            assert_eq!(a, b, "perfect correlation should duplicate the column");
        }
    }

    #[test]
    fn negative_definite_matrix_is_rejected_by_eigen() {
        let err = generate(
            "1, 2; 2, 1", "", 5, "covariance", "", "eigen", 1, false, 1e-8, "csv", 4, "", true,
        )
        .unwrap_err();
        assert!(err.contains("not positive semi-definite"), "{err}");
    }

    #[test]
    fn shape_and_value_errors_are_explicit() {
        let e = generate(
            "1, 0; 0, 1, 2", "", 5, "covariance", "", "cholesky", 1, false, 1e-8, "csv", 4, "",
            true,
        )
        .unwrap_err();
        assert!(e.contains("must be square"), "{e}");

        let e = generate(
            "1, 0.5; 0.4, 1", "", 5, "covariance", "", "cholesky", 1, false, 1e-8, "csv", 4, "",
            true,
        )
        .unwrap_err();
        assert!(e.contains("must be symmetric"), "{e}");

        let e = generate(
            "1, 0; 0, 1", "1, 2, 3", 5, "covariance", "", "cholesky", 1, false, 1e-8, "csv", 4, "",
            true,
        )
        .unwrap_err();
        assert!(e.contains("mean lists 3"), "{e}");

        let e = generate(
            "1, 0; 0, 1", "", 5, "covariance", "", "quantum", 1, false, 1e-8, "csv", 4, "", true,
        )
        .unwrap_err();
        assert!(e.contains("method must be"), "{e}");

        let e = generate(
            "1, oops; 0, 1", "", 5, "covariance", "", "cholesky", 1, false, 1e-8, "csv", 4, "",
            true,
        )
        .unwrap_err();
        assert!(e.contains("is not a number"), "{e}");

        let e = generate(
            "0.9, 0.5; 0.5, 0.9", "", 5, "correlation", "", "cholesky", 1, false, 1e-8, "csv", 4,
            "", true,
        )
        .unwrap_err();
        assert!(e.contains("1 on the diagonal"), "{e}");

        let e = generate(
            "1, 0; 0, 1", "", 5, "covariance", "2, 3", "cholesky", 1, false, 1e-8, "csv", 4, "",
            true,
        )
        .unwrap_err();
        assert!(e.contains("sd only applies"), "{e}");

        let e = generate(
            "1, 0; 0, 1", "", 2, "covariance", "", "cholesky", 1, true, 1e-8, "csv", 4, "", true,
        )
        .unwrap_err();
        assert!(e.contains("empirical needs more draws"), "{e}");

        let e = generate(
            "1, 0; 0, 1", "", 5, "covariance", "", "cholesky", 1, false, 1e-8, "csv", 4, "a,b,c",
            true,
        )
        .unwrap_err();
        assert!(e.contains("labels lists 3"), "{e}");
    }

    #[test]
    fn caps_are_enforced_at_the_boundary() {
        // 100000 x 2 = 200000 cells — exactly the cell cap.
        assert!(generate(
            "1, 0; 0, 1", "", MAX_SAMPLES, "covariance", "", "cholesky", 1, false, 1e-8, "csv", 2,
            "", false
        )
        .is_ok());
        let e = generate(
            "1, 0; 0, 1", "", MAX_SAMPLES + 1, "covariance", "", "cholesky", 1, false, 1e-8, "csv",
            2, "", false,
        )
        .unwrap_err();
        assert!(e.contains("above the maximum of 100000"), "{e}");
        // 3 variables x 100000 draws blows the cell cap first.
        let e = generate(
            "1,0,0; 0,1,0; 0,0,1", "", MAX_SAMPLES, "covariance", "", "cholesky", 1, false, 1e-8,
            "csv", 2, "", false,
        )
        .unwrap_err();
        assert!(e.contains("above the maximum of 200000"), "{e}");
        assert!(e.contains("Lower samples to 66666"), "{e}");
    }

    #[test]
    fn single_variable_reduces_to_a_plain_normal() {
        let out = generate(
            "9", "5", 40_000, "covariance", "", "cholesky", 11, false, 1e-8, "stats", 4, "value",
            true,
        )
        .unwrap();
        assert!(out.contains("40000 draw(s) of 1 variable(s)"), "{out}");
        assert!(out.contains("Variables: value"), "{out}");
        let mean_line = out.lines().find(|l| l.starts_with("Sample mean:")).unwrap();
        let m: f64 = mean_line.split(':').nth(1).unwrap().trim().parse().unwrap();
        assert!((m - 5.0).abs() < 0.05, "mean drifted: {m}");
    }

    #[test]
    fn structured_shorthands_expand_to_the_right_matrix() {
        // ar1(3, 0.5) is the same matrix as typing it out by hand.
        let shorthand = generate(
            "ar1(3, 0.5)", "", 6, "covariance", "", "cholesky", 2, false, 1e-8, "csv", 4, "", true,
        )
        .unwrap();
        let explicit = generate(
            "1,0.5,0.25; 0.5,1,0.5; 0.25,0.5,1", "", 6, "covariance", "", "cholesky", 2, false,
            1e-8, "csv", 4, "", true,
        )
        .unwrap();
        assert_eq!(shorthand, explicit);

        // cs(3, 0.4) puts 0.4 between every pair; iid(2) is the identity.
        let out = generate(
            "cs(3, 0.4)", "", 2, "covariance", "", "cholesky", 1, false, 1e-8, "stats", 2, "", true,
        )
        .unwrap();
        let target = out.split("Target covariance:").nth(1).unwrap();
        let row1 = target.lines().find(|l| l.starts_with("X1")).unwrap();
        assert_eq!(row1.split_whitespace().collect::<Vec<_>>(), ["X1", "1.00", "0.40", "0.40"]);
        let iid = generate(
            "iid(2)", "", 4, "covariance", "", "cholesky", 3, false, 1e-8, "csv", 4, "", true,
        )
        .unwrap();
        let plain = generate(
            "1,0; 0,1", "", 4, "covariance", "", "cholesky", 3, false, 1e-8, "csv", 4, "", true,
        )
        .unwrap();
        assert_eq!(iid, plain);

        // Shorthands are correlation matrices, so sd rescales them.
        let scaled = generate(
            "ar1(2, 0.8)", "", 3, "correlation", "10, 10", "cholesky", 4, false, 1e-8, "json", 4,
            "", true,
        )
        .unwrap();
        let v: Value = serde_json::from_str(&scaled).unwrap();
        assert_eq!(v["target"]["covariance"][0][1].as_f64().unwrap(), 80.0);
    }

    #[test]
    fn structured_shorthand_errors_are_explicit() {
        let e = generate(
            "ar1(4)", "", 5, "covariance", "", "cholesky", 1, false, 1e-8, "csv", 4, "", true,
        )
        .unwrap_err();
        assert!(e.contains("takes 2 value(s) but got 1"), "{e}");
        assert!(e.contains("ar1(4, 0.7)"), "error must show the form: {e}");

        let e = generate(
            "cs(3, 1.5)", "", 5, "covariance", "", "cholesky", 1, false, 1e-8, "csv", 4, "", true,
        )
        .unwrap_err();
        assert!(e.contains("outside the valid range -1 to 1"), "{e}");

        let e = generate(
            "ar1(99, 0.5)", "", 5, "covariance", "", "cholesky", 1, false, 1e-8, "csv", 4, "", true,
        )
        .unwrap_err();
        assert!(e.contains("between 1 and 50"), "{e}");

        // A negative compound-symmetry rho below -1/(d-1) is not a valid matrix.
        let e = generate(
            "cs(3, -0.9)", "", 5, "covariance", "", "eigen", 1, false, 1e-8, "csv", 4, "", true,
        )
        .unwrap_err();
        assert!(e.contains("not positive semi-definite"), "{e}");
    }

    #[test]
    fn decimals_controls_precision() {
        let out = generate(
            "1, 0; 0, 1", "", 1, "covariance", "", "cholesky", 4, false, 1e-8, "csv", 0, "", false,
        )
        .unwrap();
        assert!(
            !out.contains('.'),
            "decimals=0 must emit whole numbers: {out}"
        );
        let e = generate(
            "1, 0; 0, 1", "", 1, "covariance", "", "cholesky", 4, false, 1e-8, "csv", 13, "",
            false,
        )
        .unwrap_err();
        assert!(e.contains("decimals must be between 0 and 12"), "{e}");
    }
}
