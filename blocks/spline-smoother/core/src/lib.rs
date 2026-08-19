//! spline-smoother core — a cubic smoothing spline fitted with the classical
//! Reinsch / Green–Silverman banded solve.
//!
//! Minimises `Σ wᵢ (yᵢ − g(xᵢ))² + λ ∫ g″(x)² dx` over natural cubic splines
//! with a knot at every distinct x. The penalty matrix is `R + λ Qᵀ W⁻¹ Q`,
//! symmetric positive-definite with half-bandwidth 2, so the fit, the hat-matrix
//! diagonal (needed for effective degrees of freedom, GCV and leave-one-out CV)
//! and the inverse's central bands are all O(n).
//!
//! Pure compute — no wafer / wasm-bindgen deps. Shared by the chat skill block,
//! the `gizza` CLI and the browser page.

/// Max distinct data points accepted (the solve is O(n), the cap keeps a
/// browser tab responsive and bounds the JSON payload).
pub const MAX_POINTS: usize = 10_000;
/// Max points the `resample` curve may contain.
pub const MAX_RESAMPLE: usize = 5_000;
/// Max x values `predict_at` may contain.
pub const MAX_PREDICT: usize = 5_000;
/// Max bytes of `data` text accepted.
pub const MAX_BYTES: usize = 2_000_000;

/// Everything except the data itself. `Default` matches the descriptor's
/// declared defaults, so the CLI, the chat schema and the page agree.
#[derive(Debug, Clone)]
pub struct Options {
    pub mode: String,
    pub smoothing: f64,
    pub lambda: f64,
    pub df: f64,
    pub criterion: String,
    pub weights: String,
    pub predict_at: String,
    pub resample: usize,
    pub coefficients: bool,
    pub output: String,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            mode: "auto".into(),
            smoothing: 0.99,
            lambda: 1.0,
            df: 5.0,
            criterion: "gcv".into(),
            weights: String::new(),
            predict_at: String::new(),
            resample: 0,
            coefficients: false,
            output: "json".into(),
        }
    }
}

// ---------------------------------------------------------------------------
// Number formatting
// ---------------------------------------------------------------------------

/// Round to 6 significant digits. Every reported number goes through this so
/// the CLI, the page and the chat block print byte-identical results and tiny
/// last-bit differences never leak into the output.
fn r6(x: f64) -> f64 {
    if !x.is_finite() || x == 0.0 {
        return if x.is_finite() { 0.0 } else { x };
    }
    let mag = x.abs().log10().floor();
    let f = 10f64.powf(5.0 - mag);
    if !f.is_finite() || f == 0.0 {
        return x;
    }
    let v = (x * f).round() / f;
    if v == 0.0 {
        0.0
    } else {
        v
    }
}

/// A JSON number token (or `null` for a non-finite value).
fn jf(x: f64) -> String {
    if !x.is_finite() {
        return "null".into();
    }
    num(x)
}

/// A plain decimal token, switching to exponent form only where the decimal
/// form would be unreadable. Both forms are valid JSON numbers.
fn num(x: f64) -> String {
    let v = r6(x);
    if v == 0.0 {
        return "0".into();
    }
    let a = v.abs();
    if !(1e-4..1e15).contains(&a) {
        format!("{v:e}")
    } else {
        format!("{v}")
    }
}

/// Short label form for SVG axis ticks (4 significant digits).
fn tick(x: f64) -> String {
    if !x.is_finite() {
        return String::new();
    }
    if x == 0.0 {
        return "0".into();
    }
    let mag = x.abs().log10().floor();
    let f = 10f64.powf(3.0 - mag);
    let v = if f.is_finite() && f != 0.0 {
        (x * f).round() / f
    } else {
        x
    };
    let a = v.abs();
    if !(1e-3..1e7).contains(&a) {
        format!("{v:e}")
    } else {
        format!("{v}")
    }
}

// ---------------------------------------------------------------------------
// Input parsing
// ---------------------------------------------------------------------------

fn split_fields(line: &str) -> Vec<&str> {
    line.split(|c: char| c == ',' || c == ';' || c == '\t' || c.is_whitespace())
        .filter(|f| !f.is_empty())
        .collect()
}

fn parse_num(tok: &str, what: &str) -> Result<f64, String> {
    let v: f64 = tok
        .parse()
        .map_err(|_| format!("{what}: '{tok}' is not a number"))?;
    if !v.is_finite() {
        return Err(format!("{what}: '{tok}' is not a finite number"));
    }
    Ok(v)
}

/// A whitespace/comma/semicolon separated list of finite numbers.
fn parse_list(s: &str, what: &str) -> Result<Vec<f64>, String> {
    let mut out = Vec::new();
    for tok in s
        .split(|c: char| c == ',' || c == ';' || c.is_whitespace())
        .filter(|t| !t.is_empty())
    {
        out.push(parse_num(tok, what)?);
    }
    Ok(out)
}

fn parse_json_series(t: &str) -> Result<Vec<(f64, f64)>, String> {
    let v: serde_json::Value =
        serde_json::from_str(t).map_err(|e| format!("data is not valid JSON: {e}"))?;
    let arr = v.as_array().ok_or(
        "JSON data must be an array of numbers, [x, y] pairs, or {\"x\":…,\"y\":…} objects",
    )?;
    let mut out = Vec::with_capacity(arr.len());
    for (i, el) in arr.iter().enumerate() {
        let idx = i + 1;
        match el {
            serde_json::Value::Number(_) => {
                let y = el
                    .as_f64()
                    .filter(|v| v.is_finite())
                    .ok_or(format!("data element {idx} is not a finite number"))?;
                out.push((idx as f64, y));
            }
            serde_json::Value::Array(p) => {
                if p.len() < 2 {
                    return Err(format!("data element {idx} must be an [x, y] pair"));
                }
                let x = p[0]
                    .as_f64()
                    .filter(|v| v.is_finite())
                    .ok_or(format!("data element {idx}: x is not a finite number"))?;
                let y = p[1]
                    .as_f64()
                    .filter(|v| v.is_finite())
                    .ok_or(format!("data element {idx}: y is not a finite number"))?;
                out.push((x, y));
            }
            serde_json::Value::Object(o) => {
                let x = o
                    .get("x")
                    .and_then(|v| v.as_f64())
                    .filter(|v| v.is_finite())
                    .ok_or(format!("data element {idx}: missing a finite \"x\""))?;
                let y = o
                    .get("y")
                    .and_then(|v| v.as_f64())
                    .filter(|v| v.is_finite())
                    .ok_or(format!("data element {idx}: missing a finite \"y\""))?;
                out.push((x, y));
            }
            _ => {
                return Err(format!(
                    "data element {idx} must be a number, an [x, y] pair, or an object with x and y"
                ))
            }
        }
    }
    Ok(out)
}

fn parse_text_series(t: &str) -> Result<Vec<(f64, f64)>, String> {
    let mut lines: Vec<&str> = t
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();
    if lines.is_empty() {
        return Err("data has no rows".into());
    }
    // A first row containing any non-numeric field is a header.
    let first = split_fields(lines[0]);
    if first.is_empty() || first.iter().any(|f| f.parse::<f64>().is_err()) {
        lines.remove(0);
        if lines.is_empty() {
            return Err("data has a header row but no data rows".into());
        }
    }
    let mut rows: Vec<Vec<f64>> = Vec::with_capacity(lines.len());
    for (i, line) in lines.iter().enumerate() {
        let fields = split_fields(line);
        let mut row = Vec::with_capacity(fields.len());
        for f in fields {
            row.push(parse_num(f, &format!("data row {}", i + 1))?);
        }
        if row.is_empty() {
            return Err(format!("data row {} is empty", i + 1));
        }
        rows.push(row);
    }
    // A single row of 3+ numbers is a one-line y series ("10, 12, 11, 13").
    if rows.len() == 1 && rows[0].len() >= 3 {
        return Ok(rows[0]
            .iter()
            .enumerate()
            .map(|(i, &y)| ((i + 1) as f64, y))
            .collect());
    }
    let width = rows[0].len();
    if let Some(bad) = rows.iter().position(|r| r.len() != width) {
        return Err(format!(
            "data row {} has {} columns but row 1 has {width} — every row needs the same columns",
            bad + 1,
            rows[bad].len()
        ));
    }
    Ok(if width == 1 {
        rows.iter()
            .enumerate()
            .map(|(i, r)| ((i + 1) as f64, r[0]))
            .collect()
    } else {
        rows.iter().map(|r| (r[0], r[1])).collect()
    })
}

fn parse_series(s: &str) -> Result<Vec<(f64, f64)>, String> {
    let t = s.trim();
    if t.is_empty() {
        return Err(
            "data is empty — paste one y per line, `x,y` rows, or a JSON array of numbers".into(),
        );
    }
    if t.starts_with('[') {
        parse_json_series(t)
    } else {
        parse_text_series(t)
    }
}

// ---------------------------------------------------------------------------
// Banded linear algebra (symmetric, half-bandwidth 2)
// ---------------------------------------------------------------------------

/// `B = L D Lᵀ` for a symmetric banded `B` given by its three bands.
/// Returns `(D, l1, l2)` with `l1[j] = L[j+1][j]`, `l2[j] = L[j+2][j]`.
fn ldl_band(b0: &[f64], b1: &[f64], b2: &[f64]) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let m = b0.len();
    let mut dg = vec![0.0; m];
    let mut l1 = vec![0.0; m];
    let mut l2 = vec![0.0; m];
    for j in 0..m {
        let mut d = b0[j];
        if j >= 1 {
            d -= l1[j - 1] * l1[j - 1] * dg[j - 1];
        }
        if j >= 2 {
            d -= l2[j - 2] * l2[j - 2] * dg[j - 2];
        }
        dg[j] = d;
        if j + 1 < m {
            let mut s = b1[j];
            if j >= 1 {
                s -= l2[j - 1] * l1[j - 1] * dg[j - 1];
            }
            l1[j] = s / dg[j];
        }
        if j + 2 < m {
            l2[j] = b2[j] / dg[j];
        }
    }
    (dg, l1, l2)
}

fn ldl_solve(dg: &[f64], l1: &[f64], l2: &[f64], rhs: &[f64]) -> Vec<f64> {
    let m = dg.len();
    let mut z = vec![0.0; m];
    for j in 0..m {
        let mut v = rhs[j];
        if j >= 1 {
            v -= l1[j - 1] * z[j - 1];
        }
        if j >= 2 {
            v -= l2[j - 2] * z[j - 2];
        }
        z[j] = v;
    }
    for j in 0..m {
        z[j] /= dg[j];
    }
    for j in (0..m).rev() {
        let mut v = z[j];
        if j + 1 < m {
            v -= l1[j] * z[j + 1];
        }
        if j + 2 < m {
            v -= l2[j] * z[j + 2];
        }
        z[j] = v;
    }
    z
}

/// The central five bands of `B⁻¹` from its LDLᵀ factors (Takahashi's
/// recurrence). Only `|j−k| ≤ 2` entries are needed for the hat diagonal, and
/// they close under the recurrence, so this stays O(m).
fn band_inverse(dg: &[f64], l1: &[f64], l2: &[f64]) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let m = dg.len();
    let mut s0 = vec![0.0; m];
    let mut s1 = vec![0.0; m];
    let mut s2 = vec![0.0; m];
    for j in (0..m).rev() {
        let a1 = if j + 1 < m { l1[j] } else { 0.0 };
        let a2 = if j + 2 < m { l2[j] } else { 0.0 };
        if j + 2 < m {
            s2[j] = -a1 * s1[j + 1] - a2 * s0[j + 2];
        }
        if j + 1 < m {
            let s_j2_j1 = if j + 2 < m { s1[j + 1] } else { 0.0 };
            s1[j] = -a1 * s0[j + 1] - a2 * s_j2_j1;
        }
        s0[j] = 1.0 / dg[j] - a1 * s1[j] - a2 * s2[j];
    }
    (s0, s1, s2)
}

fn sval(s0: &[f64], s1: &[f64], s2: &[f64], j: usize, k: usize) -> f64 {
    let (a, b) = if j <= k { (j, k) } else { (k, j) };
    match b - a {
        0 => s0[a],
        1 => s1[a],
        2 => s2[a],
        _ => 0.0,
    }
}

// ---------------------------------------------------------------------------
// The fit
// ---------------------------------------------------------------------------

struct Fit {
    /// Penalty in the input's own x units. `INFINITY` = the linear limit.
    lambda: f64,
    fitted: Vec<f64>,
    /// diag(A), the hat-matrix leverages.
    lev: Vec<f64>,
    df: f64,
    rss: f64,
    roughness: f64,
    /// Second derivatives at every knot (zero at both ends — a natural spline).
    gamma: Vec<f64>,
}

impl Fit {
    fn gcv(&self, n: usize) -> Option<f64> {
        let nf = n as f64;
        let d = 1.0 - self.df / nf;
        if d <= 1e-9 {
            None
        } else {
            Some((self.rss / nf) / (d * d))
        }
    }

    fn cv(&self, y: &[f64], w: &[f64]) -> Option<f64> {
        let mut s = 0.0;
        for i in 0..y.len() {
            let d = 1.0 - self.lev[i];
            if d <= 1e-9 {
                return None;
            }
            let r = (y[i] - self.fitted[i]) / d;
            s += w[i] * r * r;
        }
        Some(s / y.len() as f64)
    }

    fn score(&self, y: &[f64], w: &[f64], criterion: &str) -> Option<f64> {
        if criterion == "cv" {
            self.cv(y, w)
        } else {
            self.gcv(y.len())
        }
    }
}

fn fit_spline(x: &[f64], y: &[f64], w: &[f64], lambda: f64) -> Fit {
    let n = x.len();
    let m = n - 2;
    let h: Vec<f64> = (0..n - 1).map(|i| x[i + 1] - x[i]).collect();

    // Column j of Q touches rows j, j+1, j+2 with these coefficients.
    let qa: Vec<f64> = (0..m).map(|j| 1.0 / h[j]).collect();
    let qc: Vec<f64> = (0..m).map(|j| 1.0 / h[j + 1]).collect();
    let qb: Vec<f64> = (0..m).map(|j| -(qa[j] + qc[j])).collect();

    // R: symmetric tridiagonal, ∫g″² = cᵀ R c.
    let r0: Vec<f64> = (0..m).map(|j| (h[j] + h[j + 1]) / 3.0).collect();
    let r1: Vec<f64> = (0..m - 1).map(|j| h[j + 1] / 6.0).collect();

    // B = R + λ Qᵀ W⁻¹ Q.
    let mut b0 = vec![0.0; m];
    let mut b1 = vec![0.0; m - 1];
    let mut b2 = vec![0.0; m.saturating_sub(2)];
    for j in 0..m {
        b0[j] = r0[j]
            + lambda * (qa[j] * qa[j] / w[j] + qb[j] * qb[j] / w[j + 1] + qc[j] * qc[j] / w[j + 2]);
    }
    for j in 0..m - 1 {
        b1[j] = r1[j] + lambda * (qb[j] * qa[j + 1] / w[j + 1] + qc[j] * qb[j + 1] / w[j + 2]);
    }
    for j in 0..m.saturating_sub(2) {
        b2[j] = lambda * (qc[j] * qa[j + 2] / w[j + 2]);
    }

    let rhs: Vec<f64> = (0..m)
        .map(|j| qa[j] * y[j] + qb[j] * y[j + 1] + qc[j] * y[j + 2])
        .collect();

    let (dg, l1, l2) = ldl_band(&b0, &b1, &b2);
    let c = ldl_solve(&dg, &l1, &l2, &rhs);
    let (s0, s1, s2) = band_inverse(&dg, &l1, &l2);

    let mut fitted = vec![0.0; n];
    let mut lev = vec![0.0; n];
    let mut terms: Vec<(usize, f64)> = Vec::with_capacity(3);
    for i in 0..n {
        terms.clear();
        if i < m {
            terms.push((i, qa[i]));
        }
        if (1..=m).contains(&i) {
            terms.push((i - 1, qb[i - 1]));
        }
        if (2..m + 2).contains(&i) {
            terms.push((i - 2, qc[i - 2]));
        }
        let qcv: f64 = terms.iter().map(|&(j, co)| co * c[j]).sum();
        fitted[i] = y[i] - lambda / w[i] * qcv;
        let mut qsq = 0.0;
        for &(j, cj) in &terms {
            for &(k, ck) in &terms {
                qsq += cj * ck * sval(&s0, &s1, &s2, j, k);
            }
        }
        lev[i] = 1.0 - lambda / w[i] * qsq;
    }

    let mut roughness = 0.0;
    for j in 0..m {
        roughness += r0[j] * c[j] * c[j];
        if j + 1 < m {
            roughness += 2.0 * r1[j] * c[j] * c[j + 1];
        }
    }

    let rss = (0..n)
        .map(|i| {
            let r = y[i] - fitted[i];
            w[i] * r * r
        })
        .sum();
    let df = lev.iter().sum();

    let mut gamma = vec![0.0; n];
    gamma[1..(m + 1)].copy_from_slice(&c[..m]);

    Fit {
        lambda,
        fitted,
        lev,
        df,
        rss,
        roughness: roughness.max(0.0),
        gamma,
    }
}

/// The λ → ∞ limit: the weighted least-squares straight line (2 degrees of
/// freedom, zero roughness).
fn fit_linear(x: &[f64], y: &[f64], w: &[f64]) -> Fit {
    let n = x.len();
    let (mut sw, mut sx, mut sxx, mut sy, mut sxy) = (0.0, 0.0, 0.0, 0.0, 0.0);
    for i in 0..n {
        sw += w[i];
        sx += w[i] * x[i];
        sxx += w[i] * x[i] * x[i];
        sy += w[i] * y[i];
        sxy += w[i] * x[i] * y[i];
    }
    let det = sw * sxx - sx * sx;
    let slope = (sw * sxy - sx * sy) / det;
    let icept = (sy - slope * sx) / sw;
    let fitted: Vec<f64> = x.iter().map(|&xi| icept + slope * xi).collect();
    let lev: Vec<f64> = (0..n)
        .map(|i| w[i] * (sxx - 2.0 * x[i] * sx + x[i] * x[i] * sw) / det)
        .collect();
    let rss = (0..n)
        .map(|i| {
            let r = y[i] - fitted[i];
            w[i] * r * r
        })
        .sum();
    let df = lev.iter().sum();
    Fit {
        lambda: f64::INFINITY,
        fitted,
        lev,
        df,
        rss,
        roughness: 0.0,
        gamma: vec![0.0; n],
    }
}

/// Value and first derivative of the fitted spline at `t`. Outside the data
/// range the natural spline continues linearly with the end slope.
fn eval_spline(x: &[f64], g: &[f64], gam: &[f64], t: f64) -> (f64, f64) {
    let n = x.len();
    if t <= x[0] {
        let h = x[1] - x[0];
        let slope = (g[1] - g[0]) / h - h * (2.0 * gam[0] + gam[1]) / 6.0;
        return (g[0] + slope * (t - x[0]), slope);
    }
    if t >= x[n - 1] {
        let h = x[n - 1] - x[n - 2];
        let slope = (g[n - 1] - g[n - 2]) / h + h * (gam[n - 2] + 2.0 * gam[n - 1]) / 6.0;
        return (g[n - 1] + slope * (t - x[n - 1]), slope);
    }
    let (mut lo, mut hi) = (0usize, n - 1);
    while hi - lo > 1 {
        let mid = (lo + hi) / 2;
        if x[mid] <= t {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let i = lo;
    let h = x[i + 1] - x[i];
    let a = (x[i + 1] - t) / h;
    let b = (t - x[i]) / h;
    let val = a * g[i]
        + b * g[i + 1]
        + ((a * a * a - a) * gam[i] + (b * b * b - b) * gam[i + 1]) * h * h / 6.0;
    let der = (g[i + 1] - g[i]) / h
        + ((-3.0 * a * a + 1.0) * gam[i] + (3.0 * b * b - 1.0) * gam[i + 1]) * h / 6.0;
    (val, der)
}

// ---------------------------------------------------------------------------
// Report assembly
// ---------------------------------------------------------------------------

struct Report {
    mode: String,
    criterion: String,
    lambda: f64,
    p: f64,
    df: f64,
    n_points: usize,
    n_input: usize,
    merged: usize,
    rss: f64,
    rmse: f64,
    roughness: f64,
    penalized: f64,
    gcv: Option<f64>,
    cv: Option<f64>,
    xs: Vec<f64>,
    ys: Vec<f64>,
    ws: Vec<f64>,
    fitted: Vec<f64>,
    lev: Vec<f64>,
    gamma: Vec<f64>,
    preds: Vec<(f64, f64, f64)>,
    curve: Vec<(f64, f64, f64)>,
    pieces: Vec<[f64; 6]>,
}

/// Fit a cubic smoothing spline to `data` and render the requested report.
pub fn smooth(data: &str, o: &Options) -> Result<String, String> {
    if data.len() > MAX_BYTES {
        return Err(format!(
            "data is {} bytes — the limit is {MAX_BYTES} bytes",
            data.len()
        ));
    }
    if !matches!(o.output.as_str(), "json" | "csv" | "svg") {
        return Err(format!(
            "output must be json, csv, or svg (got '{}')",
            o.output
        ));
    }
    if !matches!(o.criterion.as_str(), "gcv" | "cv") {
        return Err(format!(
            "criterion must be gcv or cv (got '{}')",
            o.criterion
        ));
    }

    let raw = parse_series(data)?;
    let n_input = raw.len();
    if n_input > MAX_POINTS {
        return Err(format!(
            "data has {n_input} points — the limit is {MAX_POINTS}"
        ));
    }

    let weights: Vec<f64> = if o.weights.trim().is_empty() {
        vec![1.0; n_input]
    } else {
        let w = parse_list(&o.weights, "weights")?;
        if w.len() != n_input {
            return Err(format!(
                "weights has {} values but data has {n_input} points",
                w.len()
            ));
        }
        if let Some(bad) = w.iter().find(|v| **v <= 0.0) {
            return Err(format!("weights must all be greater than 0 (got {bad})"));
        }
        w
    };

    // Sort by x, then merge exact duplicate x by their weighted mean.
    let mut order: Vec<usize> = (0..n_input).collect();
    order.sort_by(|&a, &b| {
        raw[a]
            .0
            .partial_cmp(&raw[b].0)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let (mut xs, mut ys, mut ws) = (Vec::new(), Vec::new(), Vec::new());
    let mut i = 0usize;
    while i < order.len() {
        let x = raw[order[i]].0;
        let (mut sw, mut swy) = (0.0, 0.0);
        while i < order.len() && raw[order[i]].0 == x {
            let k = order[i];
            sw += weights[k];
            swy += weights[k] * raw[k].1;
            i += 1;
        }
        xs.push(x);
        ys.push(swy / sw);
        ws.push(sw);
    }
    let n = xs.len();
    let merged = n_input - n;
    if n < 4 {
        return Err(format!(
            "a cubic smoothing spline needs at least 4 distinct x values (got {n})"
        ));
    }
    let span = xs[n - 1] - xs[0];

    // --- pick the penalty --------------------------------------------------
    // λ is searched in x-normalised units (λ_u) so the search range is
    // scale-free; the reported λ is in the input's own x units:
    // λ_x = λ_u · (xmax − xmin)³.
    let scale = span * span * span;
    let fit_u = |lu: f64| fit_spline(&xs, &ys, &ws, lu * scale);

    let fit = match o.mode.as_str() {
        "lambda" => {
            if !(o.lambda.is_finite() && o.lambda >= 0.0) {
                return Err(format!(
                    "lambda must be a finite number >= 0 (got {})",
                    o.lambda
                ));
            }
            fit_spline(&xs, &ys, &ws, o.lambda)
        }
        "smoothing" => {
            if !(o.smoothing.is_finite() && (0.0..=1.0).contains(&o.smoothing)) {
                return Err(format!(
                    "smoothing must be between 0 and 1 (got {})",
                    o.smoothing
                ));
            }
            if o.smoothing == 0.0 {
                fit_linear(&xs, &ys, &ws)
            } else {
                fit_u((1.0 - o.smoothing) / o.smoothing)
            }
        }
        "df" => {
            let target = o.df;
            if !target.is_finite() || target < 2.0 || target > n as f64 {
                return Err(format!(
                    "df must be between 2 and the number of distinct points ({n}) (got {target})"
                ));
            }
            if target >= n as f64 - 1e-9 {
                fit_spline(&xs, &ys, &ws, 0.0)
            } else if target <= 2.0 + 1e-9 {
                fit_linear(&xs, &ys, &ws)
            } else {
                // df(λ) is strictly decreasing; bisect on log10 λ_u.
                let (mut lo, mut hi) = (-14.0f64, 14.0f64);
                for _ in 0..60 {
                    let mid = 0.5 * (lo + hi);
                    if fit_u(10f64.powf(mid)).df > target {
                        lo = mid;
                    } else {
                        hi = mid;
                    }
                }
                fit_u(10f64.powf(0.5 * (lo + hi)))
            }
        }
        "auto" => {
            let score = |t: f64| fit_u(10f64.powf(t)).score(&ys, &ws, &o.criterion);
            let mut best: Option<(f64, f64)> = None;
            let mut t = -12.0f64;
            while t <= 12.0 + 1e-9 {
                if let Some(s) = score(t) {
                    if best.is_none_or(|(b, _)| s < b) {
                        best = Some((s, t));
                    }
                }
                t += 0.5;
            }
            let (_, t0) = best.ok_or(
                "could not select a smoothing parameter automatically — \
                 set mode to smoothing, lambda, or df",
            )?;
            // Golden-section refine inside the winning bracket.
            let inv = 0.618_033_988_749_895_f64;
            let (mut a, mut b) = (t0 - 0.5, t0 + 0.5);
            let val = |t: f64| score(t).unwrap_or(f64::INFINITY);
            let (mut c1, mut c2) = (b - inv * (b - a), a + inv * (b - a));
            let (mut f1, mut f2) = (val(c1), val(c2));
            for _ in 0..40 {
                if f1 <= f2 {
                    b = c2;
                    c2 = c1;
                    f2 = f1;
                    c1 = b - inv * (b - a);
                    f1 = val(c1);
                } else {
                    a = c1;
                    c1 = c2;
                    f1 = f2;
                    c2 = a + inv * (b - a);
                    f2 = val(c2);
                }
            }
            fit_u(10f64.powf(0.5 * (a + b)))
        }
        other => {
            return Err(format!(
                "mode must be auto, smoothing, lambda, or df (got '{other}')"
            ))
        }
    };

    // --- derived reporting -------------------------------------------------
    let lambda_u = if fit.lambda.is_finite() {
        fit.lambda / scale
    } else {
        f64::INFINITY
    };
    let p = if lambda_u.is_finite() {
        1.0 / (1.0 + lambda_u)
    } else {
        0.0
    };
    let rmse = (fit.rss / n as f64).sqrt();
    let penalized = if fit.lambda.is_finite() {
        fit.rss + fit.lambda * fit.roughness
    } else {
        fit.rss
    };

    // --- predictions / resampled curve / coefficients ----------------------
    let mut preds = Vec::new();
    if !o.predict_at.trim().is_empty() {
        let at = parse_list(&o.predict_at, "predict_at")?;
        if at.len() > MAX_PREDICT {
            return Err(format!(
                "predict_at has {} values — the limit is {MAX_PREDICT}",
                at.len()
            ));
        }
        for t in at {
            let (v, d) = eval_spline(&xs, &fit.fitted, &fit.gamma, t);
            preds.push((t, v, d));
        }
    }

    let mut curve = Vec::new();
    if o.resample > 0 {
        if o.resample == 1 {
            return Err("resample must be 0 (off) or at least 2".into());
        }
        if o.resample > MAX_RESAMPLE {
            return Err(format!(
                "resample is {} — the limit is {MAX_RESAMPLE}",
                o.resample
            ));
        }
        let last = o.resample - 1;
        for k in 0..o.resample {
            let t = if k == last {
                xs[n - 1]
            } else {
                xs[0] + span * (k as f64) / (last as f64)
            };
            let (v, d) = eval_spline(&xs, &fit.fitted, &fit.gamma, t);
            curve.push((t, v, d));
        }
    }

    let mut pieces = Vec::new();
    if o.coefficients {
        for i in 0..n - 1 {
            let h = xs[i + 1] - xs[i];
            let a = fit.fitted[i];
            let c = fit.gamma[i] / 2.0;
            let d = (fit.gamma[i + 1] - fit.gamma[i]) / (6.0 * h);
            let b = (fit.fitted[i + 1] - fit.fitted[i]) / h
                - h * (2.0 * fit.gamma[i] + fit.gamma[i + 1]) / 6.0;
            pieces.push([xs[i], xs[i + 1], a, b, c, d]);
        }
    }

    let rep = Report {
        mode: o.mode.clone(),
        criterion: o.criterion.clone(),
        lambda: fit.lambda,
        p,
        df: fit.df,
        n_points: n,
        n_input,
        merged,
        rss: fit.rss,
        rmse,
        roughness: fit.roughness,
        penalized,
        gcv: fit.gcv(n),
        cv: fit.cv(&ys, &ws),
        xs,
        ys,
        ws,
        fitted: fit.fitted,
        lev: fit.lev,
        gamma: fit.gamma,
        preds,
        curve,
        pieces,
    };

    Ok(match o.output.as_str() {
        "csv" => render_csv(&rep),
        "svg" => render_svg(&rep),
        _ => render_json(&rep),
    })
}

fn opt(v: Option<f64>) -> String {
    match v {
        Some(x) => jf(x),
        None => "null".into(),
    }
}

fn render_json(r: &Report) -> String {
    let mut s = String::with_capacity(256 + r.n_points * 96);
    s.push_str("{\n");
    s.push_str(&format!("  \"mode\": \"{}\",\n", r.mode));
    s.push_str(&format!("  \"criterion\": \"{}\",\n", r.criterion));
    s.push_str(&format!("  \"lambda\": {},\n", jf(r.lambda)));
    s.push_str(&format!("  \"smoothing\": {},\n", jf(r.p)));
    s.push_str(&format!("  \"effective_df\": {},\n", jf(r.df)));
    s.push_str(&format!("  \"n_points\": {},\n", r.n_points));
    s.push_str(&format!("  \"n_input\": {},\n", r.n_input));
    s.push_str(&format!("  \"merged_duplicates\": {},\n", r.merged));
    s.push_str(&format!(
        "  \"x_min\": {},\n  \"x_max\": {},\n",
        jf(r.xs[0]),
        jf(r.xs[r.n_points - 1])
    ));
    s.push_str(&format!("  \"rss\": {},\n", jf(r.rss)));
    s.push_str(&format!("  \"rmse\": {},\n", jf(r.rmse)));
    s.push_str(&format!("  \"roughness\": {},\n", jf(r.roughness)));
    s.push_str(&format!(
        "  \"penalized_criterion\": {},\n",
        jf(r.penalized)
    ));
    s.push_str(&format!("  \"gcv\": {},\n", opt(r.gcv)));
    s.push_str(&format!("  \"cv\": {},\n", opt(r.cv)));
    s.push_str("  \"points\": [\n");
    for i in 0..r.n_points {
        s.push_str(&format!(
            "    {{\"x\": {}, \"y\": {}, \"fitted\": {}, \"residual\": {}, \"weight\": {}, \"leverage\": {}}}{}\n",
            jf(r.xs[i]),
            jf(r.ys[i]),
            jf(r.fitted[i]),
            jf(r.ys[i] - r.fitted[i]),
            jf(r.ws[i]),
            jf(r.lev[i]),
            if i + 1 == r.n_points { "" } else { "," }
        ));
    }
    s.push_str("  ]");
    if !r.preds.is_empty() {
        s.push_str(",\n  \"predictions\": [\n");
        for (i, (t, v, d)) in r.preds.iter().enumerate() {
            s.push_str(&format!(
                "    {{\"x\": {}, \"y\": {}, \"slope\": {}}}{}\n",
                jf(*t),
                jf(*v),
                jf(*d),
                if i + 1 == r.preds.len() { "" } else { "," }
            ));
        }
        s.push_str("  ]");
    }
    if !r.curve.is_empty() {
        s.push_str(",\n  \"curve\": [\n");
        for (i, (t, v, d)) in r.curve.iter().enumerate() {
            s.push_str(&format!(
                "    {{\"x\": {}, \"y\": {}, \"slope\": {}}}{}\n",
                jf(*t),
                jf(*v),
                jf(*d),
                if i + 1 == r.curve.len() { "" } else { "," }
            ));
        }
        s.push_str("  ]");
    }
    if !r.pieces.is_empty() {
        s.push_str(",\n  \"pieces\": [\n");
        for (i, p) in r.pieces.iter().enumerate() {
            s.push_str(&format!(
                "    {{\"x0\": {}, \"x1\": {}, \"a\": {}, \"b\": {}, \"c\": {}, \"d\": {}}}{}\n",
                jf(p[0]),
                jf(p[1]),
                jf(p[2]),
                jf(p[3]),
                jf(p[4]),
                jf(p[5]),
                if i + 1 == r.pieces.len() { "" } else { "," }
            ));
        }
        s.push_str("  ]");
    }
    s.push_str("\n}");
    s
}

fn csv_num(x: f64) -> String {
    if x.is_finite() {
        num(x)
    } else if x.is_infinite() {
        "inf".into()
    } else {
        String::new()
    }
}

fn render_csv(r: &Report) -> String {
    let mut s = String::new();
    s.push_str("metric,value\n");
    s.push_str(&format!("mode,{}\n", r.mode));
    s.push_str(&format!("criterion,{}\n", r.criterion));
    s.push_str(&format!("lambda,{}\n", csv_num(r.lambda)));
    s.push_str(&format!("smoothing,{}\n", csv_num(r.p)));
    s.push_str(&format!("effective_df,{}\n", csv_num(r.df)));
    s.push_str(&format!("n_points,{}\n", r.n_points));
    s.push_str(&format!("n_input,{}\n", r.n_input));
    s.push_str(&format!("merged_duplicates,{}\n", r.merged));
    s.push_str(&format!("rss,{}\n", csv_num(r.rss)));
    s.push_str(&format!("rmse,{}\n", csv_num(r.rmse)));
    s.push_str(&format!("roughness,{}\n", csv_num(r.roughness)));
    s.push_str(&format!("penalized_criterion,{}\n", csv_num(r.penalized)));
    s.push_str(&format!("gcv,{}\n", r.gcv.map(csv_num).unwrap_or_default()));
    s.push_str(&format!("cv,{}\n", r.cv.map(csv_num).unwrap_or_default()));

    s.push_str("\nx,y,fitted,residual,weight,leverage\n");
    for i in 0..r.n_points {
        s.push_str(&format!(
            "{},{},{},{},{},{}\n",
            csv_num(r.xs[i]),
            csv_num(r.ys[i]),
            csv_num(r.fitted[i]),
            csv_num(r.ys[i] - r.fitted[i]),
            csv_num(r.ws[i]),
            csv_num(r.lev[i])
        ));
    }
    if !r.preds.is_empty() {
        s.push_str("\nx,predicted,slope\n");
        for (t, v, d) in &r.preds {
            s.push_str(&format!(
                "{},{},{}\n",
                csv_num(*t),
                csv_num(*v),
                csv_num(*d)
            ));
        }
    }
    if !r.curve.is_empty() {
        s.push_str("\nx,curve,slope\n");
        for (t, v, d) in &r.curve {
            s.push_str(&format!(
                "{},{},{}\n",
                csv_num(*t),
                csv_num(*v),
                csv_num(*d)
            ));
        }
    }
    if !r.pieces.is_empty() {
        s.push_str("\nx_start,x_end,a,b,c,d\n");
        for p in &r.pieces {
            s.push_str(&format!(
                "{},{},{},{},{},{}\n",
                csv_num(p[0]),
                csv_num(p[1]),
                csv_num(p[2]),
                csv_num(p[3]),
                csv_num(p[4]),
                csv_num(p[5])
            ));
        }
    }
    s.trim_end().to_string() + "\n"
}

fn render_svg(r: &Report) -> String {
    const W: f64 = 760.0;
    const H: f64 = 420.0;
    const ML: f64 = 66.0;
    const MR: f64 = 18.0;
    const MT: f64 = 34.0;
    const MB: f64 = 44.0;
    let pw = W - ML - MR;
    let ph = H - MT - MB;

    let n = r.n_points;
    let (x0, x1) = (r.xs[0], r.xs[n - 1]);
    let steps = 240usize;
    let mut curve = Vec::with_capacity(steps + 1);
    for k in 0..=steps {
        let t = x0 + (x1 - x0) * (k as f64) / (steps as f64);
        curve.push((t, eval_spline(&r.xs, &r.fitted, &r.gamma, t).0));
    }

    let mut ymin = f64::INFINITY;
    let mut ymax = f64::NEG_INFINITY;
    for &v in r.ys.iter().chain(r.fitted.iter()) {
        ymin = ymin.min(v);
        ymax = ymax.max(v);
    }
    for &(_, v) in &curve {
        ymin = ymin.min(v);
        ymax = ymax.max(v);
    }
    if !(ymax > ymin) {
        ymin -= 1.0;
        ymax += 1.0;
    }
    let pad = (ymax - ymin) * 0.06;
    ymin -= pad;
    ymax += pad;

    let sx = |t: f64| ML + (t - x0) / (x1 - x0) * pw;
    let sy = |v: f64| MT + (ymax - v) / (ymax - ymin) * ph;

    let mut s = String::with_capacity(8192);
    s.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{W}\" height=\"{H}\" viewBox=\"0 0 {W} {H}\" role=\"img\" aria-label=\"Smoothing spline fit\">\n"
    ));
    s.push_str(&format!(
        "  <rect width=\"{W}\" height=\"{H}\" fill=\"#ffffff\"/>\n"
    ));
    s.push_str(&format!(
        "  <text x=\"{ML}\" y=\"22\" font-family=\"system-ui, sans-serif\" font-size=\"13\" fill=\"#334155\">smoothing spline — df {} , lambda {} , rmse {}</text>\n",
        tick(r.df),
        if r.lambda.is_finite() { tick(r.lambda) } else { "inf".into() },
        tick(r.rmse)
    ));

    // Grid + ticks.
    for k in 0..=4 {
        let f = k as f64 / 4.0;
        let gx = ML + pw * f;
        let gy = MT + ph * f;
        s.push_str(&format!(
            "  <line x1=\"{ML}\" y1=\"{gy:.2}\" x2=\"{:.2}\" y2=\"{gy:.2}\" stroke=\"#e2e8f0\" stroke-width=\"1\"/>\n",
            ML + pw
        ));
        s.push_str(&format!(
            "  <line x1=\"{gx:.2}\" y1=\"{MT}\" x2=\"{gx:.2}\" y2=\"{:.2}\" stroke=\"#e2e8f0\" stroke-width=\"1\"/>\n",
            MT + ph
        ));
        s.push_str(&format!(
            "  <text x=\"{gx:.2}\" y=\"{:.2}\" text-anchor=\"middle\" font-family=\"system-ui, sans-serif\" font-size=\"11\" fill=\"#64748b\">{}</text>\n",
            MT + ph + 18.0,
            tick(x0 + (x1 - x0) * f)
        ));
        s.push_str(&format!(
            "  <text x=\"{:.2}\" y=\"{:.2}\" text-anchor=\"end\" font-family=\"system-ui, sans-serif\" font-size=\"11\" fill=\"#64748b\">{}</text>\n",
            ML - 8.0,
            gy + 4.0,
            tick(ymax - (ymax - ymin) * f)
        ));
    }
    s.push_str(&format!(
        "  <rect x=\"{ML}\" y=\"{MT}\" width=\"{pw}\" height=\"{ph}\" fill=\"none\" stroke=\"#94a3b8\" stroke-width=\"1\"/>\n"
    ));

    // Raw observations.
    for i in 0..n {
        s.push_str(&format!(
            "  <circle cx=\"{:.2}\" cy=\"{:.2}\" r=\"2.6\" fill=\"#94a3b8\"/>\n",
            sx(r.xs[i]),
            sy(r.ys[i])
        ));
    }
    // Fitted curve.
    let mut d = String::with_capacity(steps * 16);
    for (k, &(t, v)) in curve.iter().enumerate() {
        d.push_str(&format!(
            "{}{:.2} {:.2}",
            if k == 0 { "M" } else { " L" },
            sx(t),
            sy(v)
        ));
    }
    s.push_str(&format!(
        "  <path d=\"{d}\" fill=\"none\" stroke=\"#2563eb\" stroke-width=\"2\" stroke-linejoin=\"round\"/>\n"
    ));
    s.push_str("  <circle cx=\"640\" cy=\"20\" r=\"3\" fill=\"#94a3b8\"/>\n");
    s.push_str("  <text x=\"650\" y=\"24\" font-family=\"system-ui, sans-serif\" font-size=\"11\" fill=\"#64748b\">data</text>\n");
    s.push_str("  <line x1=\"686\" y1=\"20\" x2=\"706\" y2=\"20\" stroke=\"#2563eb\" stroke-width=\"2\"/>\n");
    s.push_str("  <text x=\"710\" y=\"24\" font-family=\"system-ui, sans-serif\" font-size=\"11\" fill=\"#64748b\">fit</text>\n");
    s.push_str("</svg>\n");
    s
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(mode: &str) -> Options {
        Options {
            mode: mode.into(),
            ..Default::default()
        }
    }

    fn field(json: &str, key: &str) -> f64 {
        let v: serde_json::Value = serde_json::from_str(json).expect("valid JSON");
        v[key].as_f64().unwrap_or_else(|| panic!("{key} missing"))
    }

    const NOISY: &str =
        "1,2.1\n2,3.9\n3,6.2\n4,7.8\n5,10.3\n6,11.7\n7,14.2\n8,15.8\n9,18.3\n10,19.7";

    #[test]
    fn happy_path_auto_gcv_returns_a_usable_fit() {
        let out = smooth(NOISY, &opts("auto")).expect("fit");
        let df = field(&out, "effective_df");
        assert!(
            (2.0..=10.0).contains(&df),
            "effective_df in [2, n], got {df}"
        );
        assert!(out.contains("\"points\": ["));
        assert!(field(&out, "rmse") >= 0.0);
        assert_eq!(field(&out, "n_points"), 10.0);
    }

    #[test]
    fn error_needs_at_least_four_distinct_x() {
        let err = smooth("1,1\n2,2\n3,3", &opts("auto")).unwrap_err();
        assert!(err.contains("at least 4 distinct x"), "{err}");
    }

    #[test]
    fn error_on_empty_and_non_numeric_data() {
        assert!(smooth("   ", &opts("auto")).unwrap_err().contains("empty"));
        let err = smooth("1,2\n2,x\n3,4\n4,5", &opts("auto")).unwrap_err();
        assert!(err.contains("'x' is not a number"), "{err}");
    }

    #[test]
    fn smoothing_one_interpolates_the_data_exactly() {
        let mut o = opts("smoothing");
        o.smoothing = 1.0;
        let out = smooth(NOISY, &o).expect("fit");
        assert_eq!(field(&out, "rss"), 0.0);
        assert_eq!(field(&out, "effective_df"), 10.0);
        assert_eq!(field(&out, "lambda"), 0.0);
    }

    #[test]
    fn smoothing_zero_is_the_least_squares_straight_line() {
        let mut o = opts("smoothing");
        o.smoothing = 0.0;
        let out = smooth(NOISY, &o).expect("fit");
        assert_eq!(field(&out, "effective_df"), 2.0);
        assert_eq!(field(&out, "roughness"), 0.0);
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert!(
            v["lambda"].is_null(),
            "the linear limit has no finite lambda"
        );
        // Fitted values must be collinear.
        let pts = v["points"].as_array().unwrap();
        let step = pts[1]["fitted"].as_f64().unwrap() - pts[0]["fitted"].as_f64().unwrap();
        for w in pts.windows(2) {
            let d = w[1]["fitted"].as_f64().unwrap() - w[0]["fitted"].as_f64().unwrap();
            assert!((d - step).abs() < 1e-4, "collinear fitted values");
        }
    }

    #[test]
    fn exactly_linear_data_is_reproduced_for_any_lambda() {
        // A straight line has zero roughness AND zero residual, so it is the
        // exact minimiser at every λ — a strong end-to-end check of the solve.
        let data = "0,1\n1,3\n2,5\n3,7\n4,9\n5,11";
        for lam in ["0", "0.001", "1", "1000"] {
            let mut o = opts("lambda");
            o.lambda = lam.parse().unwrap();
            let out = smooth(data, &o).expect("fit");
            assert!(field(&out, "rss") < 1e-18, "lambda={lam}: {out}");
            assert!(field(&out, "roughness") < 1e-12, "lambda={lam}");
        }
    }

    #[test]
    fn df_mode_hits_its_target_effective_df() {
        let mut o = opts("df");
        o.df = 4.0;
        let out = smooth(NOISY, &o).expect("fit");
        assert!((field(&out, "effective_df") - 4.0).abs() < 1e-4, "{out}");
    }

    #[test]
    fn df_out_of_range_is_rejected() {
        let mut o = opts("df");
        o.df = 25.0;
        let err = smooth(NOISY, &o).unwrap_err();
        assert!(err.contains("df must be between 2 and"), "{err}");
    }

    #[test]
    fn cv_criterion_also_selects_a_fit() {
        let mut o = opts("auto");
        o.criterion = "cv".into();
        let out = smooth(NOISY, &o).expect("fit");
        assert!(field(&out, "effective_df") >= 2.0);
        assert!(out.contains("\"criterion\": \"cv\""));
    }

    #[test]
    fn duplicate_x_values_are_merged_by_weighted_mean() {
        let mut o = opts("smoothing");
        o.smoothing = 1.0;
        let out = smooth("1,10\n1,20\n2,5\n3,6\n4,7", &o).expect("fit");
        assert_eq!(field(&out, "merged_duplicates"), 1.0);
        assert_eq!(field(&out, "n_points"), 4.0);
        assert_eq!(field(&out, "n_input"), 5.0);
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["points"][0]["y"].as_f64().unwrap(), 15.0);
        assert_eq!(v["points"][0]["weight"].as_f64().unwrap(), 2.0);
    }

    #[test]
    fn weights_must_match_the_point_count_and_be_positive() {
        let mut o = opts("auto");
        o.weights = "1,1,1".into();
        assert!(smooth(NOISY, &o).unwrap_err().contains("weights has 3"));
        o.weights = "1 1 1 1 1 1 1 1 1 0".into();
        assert!(smooth(NOISY, &o).unwrap_err().contains("greater than 0"));
    }

    #[test]
    fn predictions_and_resampling_evaluate_the_fitted_curve() {
        let mut o = opts("smoothing");
        o.smoothing = 1.0;
        o.predict_at = "1, 5.5, 10".into();
        o.resample = 5;
        let out = smooth(NOISY, &o).expect("fit");
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        let preds = v["predictions"].as_array().unwrap();
        assert_eq!(preds.len(), 3);
        // At smoothing=1 the curve interpolates, so a prediction at a knot
        // returns that knot's y exactly.
        assert_eq!(preds[0]["y"].as_f64().unwrap(), 2.1);
        assert_eq!(preds[2]["y"].as_f64().unwrap(), 19.7);
        let curve = v["curve"].as_array().unwrap();
        assert_eq!(curve.len(), 5);
        assert_eq!(curve[0]["x"].as_f64().unwrap(), 1.0);
        assert_eq!(curve[4]["x"].as_f64().unwrap(), 10.0);
    }

    #[test]
    fn coefficients_reconstruct_the_curve_between_knots() {
        let mut o = opts("smoothing");
        o.smoothing = 0.9;
        o.coefficients = true;
        o.predict_at = "2.5".into();
        let out = smooth(NOISY, &o).expect("fit");
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        let p = &v["pieces"][1]; // the piece starting at x = 2
        let (x0, a, b, c, d) = (
            p["x0"].as_f64().unwrap(),
            p["a"].as_f64().unwrap(),
            p["b"].as_f64().unwrap(),
            p["c"].as_f64().unwrap(),
            p["d"].as_f64().unwrap(),
        );
        let t = 2.5 - x0;
        let from_poly = a + b * t + c * t * t + d * t * t * t;
        let predicted = v["predictions"][0]["y"].as_f64().unwrap();
        assert!(
            (from_poly - predicted).abs() < 1e-4,
            "poly {from_poly} vs predicted {predicted}"
        );
    }

    #[test]
    fn csv_and_svg_outputs_render() {
        let mut o = opts("smoothing");
        o.smoothing = 0.9;
        o.output = "csv".into();
        let csv = smooth(NOISY, &o).expect("csv");
        assert!(csv.starts_with("metric,value\n"));
        assert!(csv.contains("\nx,y,fitted,residual,weight,leverage\n"));
        assert_eq!(csv.matches('\n').count(), 27);

        o.output = "svg".into();
        let svg = smooth(NOISY, &o).expect("svg");
        assert!(svg.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\""));
        assert!(svg.trim_end().ends_with("</svg>"));
        assert_eq!(svg.matches("<circle").count(), 10 + 1); // data points + legend
    }

    #[test]
    fn accepts_y_only_json_and_single_line_series() {
        let a = smooth("[2.1, 3.9, 6.2, 7.8, 10.3]", &opts("auto")).expect("json array");
        assert_eq!(field(&a, "n_points"), 5.0);
        assert_eq!(field(&a, "x_min"), 1.0);
        let b = smooth("2.1 3.9 6.2 7.8 10.3", &opts("auto")).expect("one line");
        assert_eq!(field(&b, "n_points"), 5.0);
        let c = smooth("[[1,2.1],[2,3.9],[3,6.2],[4,7.8]]", &opts("auto")).expect("pairs");
        assert_eq!(field(&c, "n_points"), 4.0);
        let d = smooth(
            "[{\"x\":1,\"y\":2.1},{\"x\":2,\"y\":3.9},{\"x\":3,\"y\":6.2},{\"x\":4,\"y\":7.8}]",
            &opts("auto"),
        )
        .expect("objects");
        assert_eq!(field(&d, "n_points"), 4.0);
    }

    #[test]
    fn header_row_is_skipped_and_unsorted_x_is_sorted() {
        let out = smooth("x,y\n4,7.8\n1,2.1\n3,6.2\n2,3.9", &opts("auto")).expect("fit");
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        let xs: Vec<f64> = v["points"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p["x"].as_f64().unwrap())
            .collect();
        assert_eq!(xs, vec![1.0, 2.0, 3.0, 4.0]);
    }

    #[test]
    fn point_cap_boundary_is_enforced() {
        let at: String = (0..MAX_POINTS).map(|i| format!("{}\n", i % 7)).collect();
        assert!(smooth(&at, &opts("smoothing")).is_ok());
        let over: String = (0..MAX_POINTS + 1)
            .map(|i| format!("{}\n", i % 7))
            .collect();
        let err = smooth(&over, &opts("smoothing")).unwrap_err();
        assert!(err.contains("the limit is 10000"), "{err}");
    }

    #[test]
    fn resample_cap_boundary_is_enforced() {
        let mut o = opts("smoothing");
        o.resample = MAX_RESAMPLE;
        assert!(smooth(NOISY, &o).is_ok());
        o.resample = MAX_RESAMPLE + 1;
        assert!(smooth(NOISY, &o).unwrap_err().contains("the limit is 5000"));
        o.resample = 1;
        assert!(smooth(NOISY, &o).unwrap_err().contains("at least 2"));
    }

    #[test]
    fn unknown_mode_output_and_criterion_are_rejected() {
        assert!(smooth(NOISY, &opts("wiggly"))
            .unwrap_err()
            .contains("mode must be auto"));
        let mut o = opts("auto");
        o.output = "pdf".into();
        assert!(smooth(NOISY, &o).unwrap_err().contains("output must be"));
        let mut o = opts("auto");
        o.criterion = "aic".into();
        assert!(smooth(NOISY, &o).unwrap_err().contains("criterion must be"));
    }

    #[test]
    fn smoothing_is_scale_invariant_in_x() {
        // The same p on x in seconds and x in days must give the same fitted
        // values (the penalty is computed on x rescaled to [0, 1]).
        let mut o = opts("smoothing");
        o.smoothing = 0.9;
        let a = smooth("1,2.1\n2,3.9\n3,6.2\n4,7.8\n5,10.3\n6,9.7", &o).expect("a");
        let b = smooth(
            "86400,2.1\n172800,3.9\n259200,6.2\n345600,7.8\n432000,10.3\n518400,9.7",
            &o,
        )
        .expect("b");
        let fa: Vec<f64> = serde_json::from_str::<serde_json::Value>(&a).unwrap()["points"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p["fitted"].as_f64().unwrap())
            .collect();
        let fb: Vec<f64> = serde_json::from_str::<serde_json::Value>(&b).unwrap()["points"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p["fitted"].as_f64().unwrap())
            .collect();
        for (x, y) in fa.iter().zip(&fb) {
            assert!((x - y).abs() < 1e-4, "{x} vs {y}");
        }
    }

    #[test]
    fn heavier_weights_pull_the_curve_toward_that_point() {
        let data = "1,0\n2,0\n3,5\n4,0\n5,0\n6,0";
        let mut o = opts("lambda");
        o.lambda = 0.5;
        let base = smooth(data, &o).expect("base");
        o.weights = "1,1,50,1,1,1".into();
        let heavy = smooth(data, &o).expect("weighted");
        let f = |s: &str| -> f64 {
            serde_json::from_str::<serde_json::Value>(s).unwrap()["points"][2]["fitted"]
                .as_f64()
                .unwrap()
        };
        assert!(f(&heavy) > f(&base), "{} vs {}", f(&heavy), f(&base));
    }
}
