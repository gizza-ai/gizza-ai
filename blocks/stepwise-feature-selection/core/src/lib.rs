//! stepwise-feature-selection core — pure compute, shared by the chat skill
//! block and the web page. No wafer/wasm-bindgen deps.
//!
//! Runs forward, backward, or bidirectional stepwise ordinary-least-squares
//! regression over a pasted numeric table: one column is the target, the rest
//! are candidate predictors, and predictors are added/removed one at a time
//! until no single move improves the chosen criterion (AIC, BIC, AICc) or until
//! every remaining term clears the p-value thresholds. The report shows the
//! selection path step by step, the final model's coefficients with standard
//! errors / t / p, its fit statistics, and a null-vs-selected-vs-full comparison.
//!
//! Math notes:
//! - Every candidate model is fitted from ONE precomputed **centred**
//!   cross-product matrix (`Σ(xᵢ−x̄ᵢ)(xⱼ−x̄ⱼ)`), so a subset fit costs O(p³)
//!   instead of O(n·p²) — backward elimination over 60 predictors stays fast.
//!   Centring also keeps the normal equations far better conditioned than a raw
//!   `XᵀX` with an intercept column.
//! - The intercept is recovered as `ȳ − Σ bⱼx̄ⱼ`, and its variance as
//!   `σ²(1/n + x̄ᵀC⁻¹x̄)`.
//! - Information criteria use the standard regression form
//!   `n·ln(RSS/n) + penalty·k` with `k` = number of fitted coefficients
//!   (intercept included) — the scale-free convention R's `step()` uses. It
//!   differs from a full log-likelihood AIC by a constant that is the same for
//!   every model on the same data, so model ORDERING is identical.
//! - t / F tail probabilities come from the regularized incomplete beta function
//!   (Numerical-Recipes `betacf` + `gammln`). Deterministic f64 throughout, no
//!   RNG, so it behaves identically under wasmi, wasm-bindgen and native.

/// Largest accepted number of data rows (observations).
pub const MAX_ROWS: usize = 20_000;
/// Largest accepted number of columns (target + candidate predictors).
pub const MAX_COLS: usize = 60;
/// Relative improvement a move must deliver before it is taken (stops
/// bidirectional search from oscillating between two equivalent models).
const IMPROVE_TOL: f64 = 1e-9;

// ---------------------------------------------------------------------------
// options
// ---------------------------------------------------------------------------

/// Every knob the tool exposes. `Default` mirrors the descriptor defaults.
#[derive(Clone, Debug)]
pub struct Options {
    /// Rows of numbers, one observation per line.
    pub data: String,
    /// Target column: `last` (default), `first`, a 1-based index, or a name.
    pub target: String,
    /// `forward`, `backward` or `both`.
    pub direction: String,
    /// `aic`, `bic`, `aicc` or `pvalue`.
    pub criterion: String,
    /// p-value below which a predictor may ENTER (used when `criterion=pvalue`).
    pub alpha_enter: f64,
    /// p-value above which a predictor is REMOVED (used when `criterion=pvalue`).
    pub alpha_remove: f64,
    /// Comma-separated predictors always kept in the model.
    pub force: String,
    /// Optional comma-separated column names.
    pub labels: String,
    /// Treat the first row as column names.
    pub header: bool,
    /// Decimal places in the report.
    pub decimals: u32,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            data: String::new(),
            target: "last".into(),
            direction: "both".into(),
            criterion: "aic".into(),
            alpha_enter: 0.05,
            alpha_remove: 0.10,
            force: String::new(),
            labels: String::new(),
            header: false,
            decimals: 4,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Direction {
    Forward,
    Backward,
    Both,
}

impl Direction {
    fn parse(s: &str) -> Result<Direction, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "both" | "bidirectional" | "stepwise" => Ok(Direction::Both),
            "forward" => Ok(Direction::Forward),
            "backward" => Ok(Direction::Backward),
            other => Err(format!(
                "direction must be 'forward', 'backward', or 'both' (got '{other}')"
            )),
        }
    }
    fn label(self) -> &'static str {
        match self {
            Direction::Forward => "forward selection",
            Direction::Backward => "backward elimination",
            Direction::Both => "bidirectional stepwise",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Criterion {
    Aic,
    Bic,
    Aicc,
    PValue,
}

impl Criterion {
    fn parse(s: &str) -> Result<Criterion, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "aic" => Ok(Criterion::Aic),
            "bic" | "sbc" => Ok(Criterion::Bic),
            "aicc" => Ok(Criterion::Aicc),
            "pvalue" | "p-value" | "p_value" | "sl" => Ok(Criterion::PValue),
            other => Err(format!(
                "criterion must be 'aic', 'bic', 'aicc', or 'pvalue' (got '{other}')"
            )),
        }
    }
    fn label(self) -> &'static str {
        match self {
            Criterion::Aic => "AIC",
            Criterion::Bic => "BIC",
            Criterion::Aicc => "AICc",
            Criterion::PValue => "p-value",
        }
    }
}

// ---------------------------------------------------------------------------
// table parsing
// ---------------------------------------------------------------------------

/// Split one row on commas, semicolons, tabs or runs of spaces.
fn split_row(line: &str) -> Vec<&str> {
    line.split(|c: char| c == ',' || c == ';' || c == '\t' || c.is_whitespace())
        .filter(|s| !s.is_empty())
        .collect()
}

struct Table {
    names: Vec<String>,
    /// Column-major values: `cols[c][r]`.
    cols: Vec<Vec<f64>>,
    n: usize,
}

fn parse_table(data: &str, header: bool, labels: &str) -> Result<Table, String> {
    let mut rows: Vec<(usize, Vec<&str>)> = Vec::new();
    for (i, raw) in data.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        rows.push((i + 1, split_row(line)));
    }
    if rows.is_empty() {
        return Err("data is empty — paste rows of numbers, one observation per line, e.g. '1,4,9'".into());
    }

    let mut names: Vec<String> = Vec::new();
    if header {
        let (_, first) = rows.remove(0);
        names = first.iter().map(|s| s.to_string()).collect();
        if rows.is_empty() {
            return Err("data has a header row but no data rows — add at least 3 rows of numbers".into());
        }
    }

    let width = rows[0].1.len();
    if width < 2 {
        return Err(format!(
            "data needs at least 2 columns (a target plus 1 candidate predictor); row {} has {width}",
            rows[0].0
        ));
    }
    if width > MAX_COLS {
        return Err(format!(
            "too many columns: {width} (max {MAX_COLS}) — drop columns before selecting"
        ));
    }
    if rows.len() > MAX_ROWS {
        return Err(format!(
            "too many rows: {} (max {MAX_ROWS}) — sample the data before selecting",
            rows.len()
        ));
    }
    if rows.len() < 3 {
        return Err(format!(
            "need at least 3 data rows to fit a regression; got {}",
            rows.len()
        ));
    }

    let mut cols: Vec<Vec<f64>> = vec![Vec::with_capacity(rows.len()); width];
    for (lineno, fields) in &rows {
        if fields.len() != width {
            return Err(format!(
                "row {lineno} has {} values but the first data row has {width} — every row needs the same number of columns",
                fields.len()
            ));
        }
        for (c, f) in fields.iter().enumerate() {
            let v: f64 = f.parse().map_err(|_| {
                format!("row {lineno}, column {}: expected a number, got '{f}'", c + 1)
            })?;
            if !v.is_finite() {
                return Err(format!(
                    "row {lineno}, column {}: expected a finite number, got '{f}'",
                    c + 1
                ));
            }
            cols[c].push(v);
        }
    }

    // Explicit `labels` override the header row; otherwise default to v1..vN.
    let explicit: Vec<String> = labels
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if !explicit.is_empty() {
        if explicit.len() != width {
            return Err(format!(
                "labels has {} names but the data has {width} columns",
                explicit.len()
            ));
        }
        names = explicit;
    } else if names.is_empty() {
        names = (1..=width).map(|i| format!("v{i}")).collect();
    } else if names.len() != width {
        return Err(format!(
            "header row has {} names but the data has {width} columns",
            names.len()
        ));
    }

    let n = rows.len();
    Ok(Table { names, cols, n })
}

/// Resolve a column reference (name, 1-based index, `first`, `last`) to an index.
fn resolve_column(spec: &str, names: &[String], what: &str) -> Result<usize, String> {
    let s = spec.trim();
    if s.is_empty() || s.eq_ignore_ascii_case("last") {
        return Ok(names.len() - 1);
    }
    if s.eq_ignore_ascii_case("first") {
        return Ok(0);
    }
    if let Some(hit) = names.iter().position(|n| n.eq_ignore_ascii_case(s)) {
        return Ok(hit);
    }
    if let Ok(i) = s.parse::<usize>() {
        if i >= 1 && i <= names.len() {
            return Ok(i - 1);
        }
        return Err(format!(
            "{what} '{s}' is out of range — the data has {} columns (use 1..{})",
            names.len(),
            names.len()
        ));
    }
    Err(format!(
        "{what} '{s}' matches no column — use a column name ({}), a 1-based index, 'first', or 'last'",
        names.join(", ")
    ))
}

// ---------------------------------------------------------------------------
// least squares from a centred cross-product matrix
// ---------------------------------------------------------------------------

/// Precomputed sufficient statistics: centred cross-products of the predictors
/// with each other and with the target.
struct Cross {
    /// `cxx[i][j] = Σ(xᵢ−x̄ᵢ)(xⱼ−x̄ⱼ)` over the predictor columns.
    cxx: Vec<Vec<f64>>,
    /// `cxy[i] = Σ(xᵢ−x̄ᵢ)(y−ȳ)`.
    cxy: Vec<f64>,
    /// `cyy = Σ(y−ȳ)²` (the total sum of squares).
    cyy: f64,
    /// Predictor means, in predictor order.
    means: Vec<f64>,
    /// Target mean.
    ymean: f64,
    n: usize,
}

fn cross_products(cols: &[Vec<f64>], preds: &[usize], target: usize) -> Cross {
    let n = cols[target].len();
    let k = preds.len();
    let mean = |c: &Vec<f64>| c.iter().sum::<f64>() / n as f64;
    let means: Vec<f64> = preds.iter().map(|&p| mean(&cols[p])).collect();
    let ymean = mean(&cols[target]);

    let mut cxx = vec![vec![0.0f64; k]; k];
    let mut cxy = vec![0.0f64; k];
    let mut cyy = 0.0f64;
    let mut row = vec![0.0f64; k];
    for r in 0..n {
        for i in 0..k {
            row[i] = cols[preds[i]][r] - means[i];
        }
        let dy = cols[target][r] - ymean;
        cyy += dy * dy;
        for i in 0..k {
            cxy[i] += row[i] * dy;
            for j in i..k {
                cxx[i][j] += row[i] * row[j];
            }
        }
    }
    for i in 0..k {
        for j in 0..i {
            cxx[i][j] = cxx[j][i];
        }
    }
    Cross {
        cxx,
        cxy,
        cyy,
        means,
        ymean,
        n,
    }
}

/// A fitted model over a subset of the predictors.
#[derive(Clone)]
struct Fit {
    /// Subset of predictor slots (indices into `Cross`), in selection order.
    subset: Vec<usize>,
    /// Intercept followed by one slope per `subset` entry.
    coefs: Vec<f64>,
    /// Standard errors, aligned with `coefs`.
    se: Vec<f64>,
    /// Residual sum of squares.
    rss: f64,
    /// Number of fitted coefficients (intercept included).
    k: usize,
    /// Residual degrees of freedom (`n − k`).
    df: usize,
}

impl Fit {
    fn r2(&self, cyy: f64) -> f64 {
        if cyy <= 0.0 {
            return 0.0;
        }
        1.0 - self.rss / cyy
    }
    fn adj_r2(&self, cyy: f64, n: usize) -> f64 {
        if cyy <= 0.0 || self.df == 0 {
            return 0.0;
        }
        1.0 - (self.rss / self.df as f64) / (cyy / (n - 1) as f64)
    }
    fn rmse(&self) -> f64 {
        (self.rss / self.df.max(1) as f64).sqrt()
    }
    fn t_stat(&self, i: usize) -> f64 {
        if self.se[i] > 0.0 {
            self.coefs[i] / self.se[i]
        } else {
            f64::NAN
        }
    }
    fn p_value(&self, i: usize) -> f64 {
        let t = self.t_stat(i);
        if !t.is_finite() || self.df == 0 {
            return f64::NAN;
        }
        student_t_two_tail(t, self.df as f64)
    }
    fn info(&self, c: Criterion, n: usize) -> f64 {
        let mse = (self.rss / n as f64).max(f64::MIN_POSITIVE);
        let base = n as f64 * mse.ln();
        let k = self.k as f64;
        match c {
            Criterion::Bic => base + k * (n as f64).ln(),
            Criterion::Aicc => {
                let denom = n as f64 - k - 1.0;
                if denom <= 0.0 {
                    f64::INFINITY
                } else {
                    base + 2.0 * k + 2.0 * k * (k + 1.0) / denom
                }
            }
            // AIC is also the reported information criterion for the p-value
            // strategy, which selects on p-values but still reports AIC.
            _ => base + 2.0 * k,
        }
    }
}

/// Gauss-Jordan inverse with partial pivoting. `None` when the matrix is
/// numerically singular (an exactly collinear / constant predictor subset).
fn invert(a: &[Vec<f64>]) -> Option<Vec<Vec<f64>>> {
    let p = a.len();
    if p == 0 {
        return Some(Vec::new());
    }
    let scale = a
        .iter()
        .flat_map(|r| r.iter())
        .fold(0.0f64, |m, v| m.max(v.abs()));
    if scale <= 0.0 {
        return None;
    }
    let mut m: Vec<Vec<f64>> = (0..p)
        .map(|i| {
            let mut row = a[i].clone();
            row.extend((0..p).map(|j| if i == j { 1.0 } else { 0.0 }));
            row
        })
        .collect();
    for c in 0..p {
        let mut piv = c;
        for r in c + 1..p {
            if m[r][c].abs() > m[piv][c].abs() {
                piv = r;
            }
        }
        if m[piv][c].abs() < 1e-11 * scale {
            return None;
        }
        m.swap(c, piv);
        let d = m[c][c];
        for v in m[c].iter_mut() {
            *v /= d;
        }
        for r in 0..p {
            if r == c {
                continue;
            }
            let f = m[r][c];
            if f == 0.0 {
                continue;
            }
            for j in 0..2 * p {
                m[r][j] -= f * m[c][j];
            }
        }
    }
    Some(m.into_iter().map(|r| r[p..].to_vec()).collect())
}

/// Fit the OLS model using exactly `subset` (may be empty = intercept only).
/// `None` when the subset is collinear or leaves no residual degrees of freedom.
fn fit_subset(cx: &Cross, subset: &[usize]) -> Option<Fit> {
    let n = cx.n;
    let k = subset.len() + 1;
    if n <= k {
        return None;
    }
    let df = n - k;

    if subset.is_empty() {
        let rss = cx.cyy;
        let se0 = (rss / df as f64 / n as f64).sqrt();
        return Some(Fit {
            subset: Vec::new(),
            coefs: vec![cx.ymean],
            se: vec![se0],
            rss,
            k,
            df,
        });
    }

    let a: Vec<Vec<f64>> = subset
        .iter()
        .map(|&i| subset.iter().map(|&j| cx.cxx[i][j]).collect())
        .collect();
    let ainv = invert(&a)?;
    let b: Vec<f64> = subset.iter().map(|&i| cx.cxy[i]).collect();
    let slopes: Vec<f64> = (0..subset.len())
        .map(|i| (0..subset.len()).map(|j| ainv[i][j] * b[j]).sum())
        .collect();

    let rss = (cx.cyy - slopes.iter().zip(&b).map(|(s, bi)| s * bi).sum::<f64>()).max(0.0);
    let sigma2 = rss / df as f64;

    // Intercept: ȳ − Σ bⱼx̄ⱼ, with var = σ²(1/n + x̄ᵀC⁻¹x̄).
    let xbar: Vec<f64> = subset.iter().map(|&i| cx.means[i]).collect();
    let intercept = cx.ymean - slopes.iter().zip(&xbar).map(|(s, m)| s * m).sum::<f64>();
    let quad: f64 = (0..subset.len())
        .map(|i| (0..subset.len()).map(|j| xbar[i] * ainv[i][j] * xbar[j]).sum::<f64>())
        .sum();
    let var0 = sigma2 * (1.0 / n as f64 + quad);

    let mut coefs = Vec::with_capacity(k);
    let mut se = Vec::with_capacity(k);
    coefs.push(intercept);
    se.push(if var0 > 0.0 { var0.sqrt() } else { 0.0 });
    for (i, s) in slopes.iter().enumerate() {
        coefs.push(*s);
        let v = sigma2 * ainv[i][i];
        se.push(if v > 0.0 { v.sqrt() } else { 0.0 });
    }

    Some(Fit {
        subset: subset.to_vec(),
        coefs,
        se,
        rss,
        k,
        df,
    })
}

// ---- special functions (Numerical Recipes) ---------------------------------

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

// ---------------------------------------------------------------------------
// the search
// ---------------------------------------------------------------------------

/// One recorded move in the selection path.
struct Step {
    /// `'+'` entered, `'-'` left, `'='` the starting model.
    action: char,
    name: String,
    /// Criterion value AFTER the move (or the moved term's p-value).
    metric: f64,
    /// Model size after the move (predictor count).
    size: usize,
}

struct Search {
    steps: Vec<Step>,
    stop_reason: String,
    fit: Fit,
}

/// Information-criterion search (AIC / BIC / AICc).
fn search_info(cx: &Cross, dir: Direction, crit: Criterion, forced: &[usize], k: usize) -> Result<Search, String> {
    let mut subset: Vec<usize> = match dir {
        Direction::Backward => (0..k).collect(),
        _ => forced.to_vec(),
    };
    let mut current = fit_subset(cx, &subset).ok_or_else(|| start_error(dir, subset.len(), cx.n))?;
    let mut steps = vec![Step {
        action: '=',
        name: start_label(&subset, forced),
        metric: current.info(crit, cx.n),
        size: subset.len(),
    }];

    let max_moves = 4 * k + 8;
    let mut stop_reason = format!("no single move improves {}", crit.label());
    for _ in 0..max_moves {
        let cur_val = current.info(crit, cx.n);
        let mut best: Option<(f64, char, usize, Fit)> = None;

        let may_add = dir != Direction::Backward;
        if may_add {
            for cand in 0..k {
                if subset.contains(&cand) {
                    continue;
                }
                let mut trial = subset.clone();
                trial.push(cand);
                if let Some(f) = fit_subset(cx, &trial) {
                    let v = f.info(crit, cx.n);
                    if best.as_ref().is_none_or(|(bv, _, _, _)| v < *bv) {
                        best = Some((v, '+', cand, f));
                    }
                }
            }
        }
        let may_drop = dir != Direction::Forward;
        if may_drop {
            for (pos, &cand) in subset.iter().enumerate() {
                if forced.contains(&cand) {
                    continue;
                }
                let mut trial = subset.clone();
                trial.remove(pos);
                if let Some(f) = fit_subset(cx, &trial) {
                    let v = f.info(crit, cx.n);
                    if best.as_ref().is_none_or(|(bv, _, _, _)| v < *bv) {
                        best = Some((v, '-', cand, f));
                    }
                }
            }
        }

        let Some((val, action, cand, f)) = best else {
            stop_reason = "no further move is possible".into();
            break;
        };
        if !(val < cur_val - IMPROVE_TOL * (1.0 + cur_val.abs())) {
            break;
        }
        if action == '+' {
            subset.push(cand);
        } else {
            subset.retain(|&c| c != cand);
        }
        current = f;
        steps.push(Step {
            action,
            // Predictor SLOT, resolved to a column name when the report is
            // rendered (the search itself never sees names).
            name: format!("#{cand}"),
            metric: val,
            size: subset.len(),
        });
    }

    Ok(Search {
        steps,
        stop_reason,
        fit: current,
    })
}

/// p-value search: enter the most significant candidate below `alpha_enter`,
/// then drop any term whose p-value exceeds `alpha_remove`.
fn search_pvalue(
    cx: &Cross,
    dir: Direction,
    forced: &[usize],
    k: usize,
    alpha_enter: f64,
    alpha_remove: f64,
) -> Result<Search, String> {
    let mut subset: Vec<usize> = match dir {
        Direction::Backward => (0..k).collect(),
        _ => forced.to_vec(),
    };
    let mut current = fit_subset(cx, &subset).ok_or_else(|| start_error(dir, subset.len(), cx.n))?;
    let mut steps = vec![Step {
        action: '=',
        name: start_label(&subset, forced),
        metric: f64::NAN,
        size: subset.len(),
    }];

    let max_moves = 4 * k + 8;
    let mut stop_reason = format!(
        "no candidate enters below alpha_enter={alpha_enter} and every term is below alpha_remove={alpha_remove}"
    );
    for _ in 0..max_moves {
        // Removal pass first (a term can turn insignificant once others enter).
        if dir != Direction::Forward {
            let worst = current
                .subset
                .iter()
                .enumerate()
                .filter(|(_, c)| !forced.contains(c))
                .map(|(i, c)| (*c, current.p_value(i + 1)))
                .filter(|(_, p)| p.is_finite())
                .fold(None::<(usize, f64)>, |acc, (c, p)| match acc {
                    Some((_, bp)) if bp >= p => acc,
                    _ => Some((c, p)),
                });
            if let Some((cand, p)) = worst {
                if p > alpha_remove {
                    subset.retain(|&c| c != cand);
                    match fit_subset(cx, &subset) {
                        Some(f) => current = f,
                        None => {
                            stop_reason = "removal left a model that cannot be fitted".into();
                            break;
                        }
                    }
                    steps.push(Step {
                        action: '-',
                        name: format!("#{cand}"),
                        metric: p,
                        size: subset.len(),
                    });
                    continue;
                }
            }
        }

        // Entry pass.
        if dir != Direction::Backward {
            let mut best: Option<(usize, f64, Fit)> = None;
            for cand in 0..k {
                if subset.contains(&cand) {
                    continue;
                }
                let mut trial = subset.clone();
                trial.push(cand);
                if let Some(f) = fit_subset(cx, &trial) {
                    let p = f.p_value(trial.len());
                    if p.is_finite() && best.as_ref().is_none_or(|(_, bp, _)| p < *bp) {
                        best = Some((cand, p, f));
                    }
                }
            }
            if let Some((cand, p, f)) = best {
                if p < alpha_enter {
                    subset.push(cand);
                    current = f;
                    steps.push(Step {
                        action: '+',
                        name: format!("#{cand}"),
                        metric: p,
                        size: subset.len(),
                    });
                    continue;
                }
            }
        }
        break;
    }

    Ok(Search {
        steps,
        stop_reason,
        fit: current,
    })
}

fn start_label(subset: &[usize], forced: &[usize]) -> String {
    if subset.is_empty() {
        "intercept only".into()
    } else if subset.len() == forced.len() {
        "forced predictors only".into()
    } else {
        "all predictors".into()
    }
}

fn start_error(dir: Direction, size: usize, n: usize) -> String {
    match dir {
        Direction::Backward => format!(
            "backward elimination starts from all {size} predictors, which needs more than {} rows (got {n}) — use direction=forward or add rows",
            size + 1
        ),
        _ => format!(
            "the starting model ({size} predictors) cannot be fitted from {n} rows — add rows or remove forced predictors"
        ),
    }
}

// ---------------------------------------------------------------------------
// report
// ---------------------------------------------------------------------------

fn fmt(v: f64, d: usize) -> String {
    if v.is_nan() {
        return "n/a".into();
    }
    if v.is_infinite() {
        return if v > 0.0 { "inf".into() } else { "-inf".into() };
    }
    format!("{v:.d$}")
}

fn pad_left(s: &str, w: usize) -> String {
    if s.len() >= w {
        s.to_string()
    } else {
        format!("{}{}", " ".repeat(w - s.len()), s)
    }
}

fn pad_right(s: &str, w: usize) -> String {
    if s.len() >= w {
        s.to_string()
    } else {
        format!("{}{}", s, " ".repeat(w - s.len()))
    }
}

/// Run the stepwise search and render the full text report.
pub fn select_report(o: &Options) -> Result<String, String> {
    let dir = Direction::parse(&o.direction)?;
    let crit = Criterion::parse(&o.criterion)?;
    let d = o.decimals.min(10) as usize;
    if !(o.alpha_enter > 0.0 && o.alpha_enter < 1.0) {
        return Err(format!(
            "alpha_enter must be greater than 0 and less than 1 (got {})",
            o.alpha_enter
        ));
    }
    if !(o.alpha_remove > 0.0 && o.alpha_remove < 1.0) {
        return Err(format!(
            "alpha_remove must be greater than 0 and less than 1 (got {})",
            o.alpha_remove
        ));
    }
    if crit == Criterion::PValue && o.alpha_remove < o.alpha_enter {
        return Err(format!(
            "alpha_remove ({}) must be at least alpha_enter ({}) — otherwise a predictor is removed the moment it enters",
            o.alpha_remove, o.alpha_enter
        ));
    }

    let table = parse_table(&o.data, o.header, &o.labels)?;
    let target = resolve_column(&o.target, &table.names, "target")?;
    let pred_cols: Vec<usize> = (0..table.names.len()).filter(|&c| c != target).collect();
    let pred_names: Vec<String> = pred_cols.iter().map(|&c| table.names[c].clone()).collect();
    let k = pred_cols.len();

    // Forced predictors, resolved into predictor-slot indices.
    let mut forced: Vec<usize> = Vec::new();
    for spec in o.force.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        let col = resolve_column(spec, &table.names, "force")?;
        if col == target {
            return Err(format!(
                "force '{spec}' is the target column ('{}') — force only applies to predictors",
                table.names[target]
            ));
        }
        let slot = pred_cols.iter().position(|&c| c == col).unwrap();
        if !forced.contains(&slot) {
            forced.push(slot);
        }
    }

    let cx = cross_products(&table.cols, &pred_cols, target);
    if cx.cyy <= 0.0 {
        return Err(format!(
            "the target column '{}' is constant — every value is {}, so there is nothing to explain",
            table.names[target],
            fmt(cx.ymean, d)
        ));
    }

    let search = match crit {
        Criterion::PValue => search_pvalue(&cx, dir, &forced, k, o.alpha_enter, o.alpha_remove)?,
        _ => search_info(&cx, dir, crit, &forced, k)?,
    };
    let fit = &search.fit;
    let name_of = |slot: usize| pred_names[slot].clone();

    let mut out = String::new();
    out.push_str(&format!(
        "Stepwise feature selection — {}, {}\n",
        dir.label(),
        crit.label()
    ));
    out.push_str(&format!(
        "Observations: {} · candidate predictors: {} · target: {}\n",
        table.n, k, table.names[target]
    ));
    if !forced.is_empty() {
        out.push_str(&format!(
            "Forced in: {}\n",
            forced.iter().map(|&s| name_of(s)).collect::<Vec<_>>().join(", ")
        ));
    }

    // ---- selection path ----
    let metric_label = if crit == Criterion::PValue {
        "p"
    } else {
        crit.label()
    };
    out.push_str("\nSelection path\n");
    for (i, s) in search.steps.iter().enumerate() {
        let label = match s.action {
            '=' => format!("start   {{{}}}", s.name),
            '+' => {
                let slot: usize = s.name.trim_start_matches('#').parse().unwrap_or(0);
                format!("step {:<2} + {}", i, name_of(slot))
            }
            _ => {
                let slot: usize = s.name.trim_start_matches('#').parse().unwrap_or(0);
                format!("step {:<2} - {}", i, name_of(slot))
            }
        };
        let metric = if s.metric.is_nan() && crit == Criterion::PValue {
            String::new()
        } else {
            format!("{metric_label} {}", fmt(s.metric, d))
        };
        out.push_str(&format!(
            "  {}  {}  ({} predictor{})\n",
            pad_right(&label, 30),
            pad_left(&metric, 14),
            s.size,
            if s.size == 1 { "" } else { "s" }
        ));
    }
    out.push_str(&format!("  stop: {}\n", search.stop_reason));

    // ---- selected model ----
    out.push_str(&format!(
        "\nSelected model ({} of {} predictors)\n",
        fit.subset.len(),
        k
    ));
    let mut eq = format!("  {} = {}", table.names[target], fmt(fit.coefs[0], d));
    for (i, &slot) in fit.subset.iter().enumerate() {
        let b = fit.coefs[i + 1];
        eq.push_str(&format!(
            " {} {}·{}",
            if b < 0.0 { "-" } else { "+" },
            fmt(b.abs(), d),
            name_of(slot)
        ));
    }
    out.push_str(&eq);
    out.push('\n');

    let mut term_names: Vec<String> = vec!["(Intercept)".into()];
    term_names.extend(fit.subset.iter().map(|&s| name_of(s)));
    let name_w = term_names.iter().map(|s| s.len()).max().unwrap_or(11).max(11);
    let num_w = (d + 8).max(10);
    out.push_str(&format!(
        "\n  {}{}{}{}{}\n",
        pad_right("term", name_w + 2),
        pad_left("estimate", num_w),
        pad_left("std error", num_w),
        pad_left("t stat", num_w),
        pad_left("p value", num_w)
    ));
    for (i, tn) in term_names.iter().enumerate() {
        out.push_str(&format!(
            "  {}{}{}{}{}\n",
            pad_right(tn, name_w + 2),
            pad_left(&fmt(fit.coefs[i], d), num_w),
            pad_left(&fmt(fit.se[i], d), num_w),
            pad_left(&fmt(fit.t_stat(i), d), num_w),
            pad_left(&fmt(fit.p_value(i), d), num_w)
        ));
    }

    // ---- fit statistics ----
    out.push_str("\nFit\n");
    out.push_str(&format!(
        "  R² {} · adjusted R² {} · RMSE {} · residual df {}\n",
        fmt(fit.r2(cx.cyy), d),
        fmt(fit.adj_r2(cx.cyy, cx.n), d),
        fmt(fit.rmse(), d),
        fit.df
    ));
    out.push_str(&format!(
        "  AIC {} · BIC {} · AICc {}\n",
        fmt(fit.info(Criterion::Aic, cx.n), d),
        fmt(fit.info(Criterion::Bic, cx.n), d),
        fmt(fit.info(Criterion::Aicc, cx.n), d)
    ));
    if !fit.subset.is_empty() {
        let d1 = fit.subset.len() as f64;
        let d2 = fit.df as f64;
        let f = ((cx.cyy - fit.rss) / d1) / (fit.rss / d2);
        out.push_str(&format!(
            "  F({}, {}) = {}, p = {}\n",
            fit.subset.len(),
            fit.df,
            fmt(f, d),
            fmt(f_upper_tail(f, d1, d2), d)
        ));
    } else {
        out.push_str("  no predictors were selected — the intercept-only model wins\n");
    }

    // ---- dropped ----
    let dropped: Vec<String> = (0..k)
        .filter(|s| !fit.subset.contains(s))
        .map(name_of)
        .collect();
    out.push_str(&format!(
        "\nDropped ({}): {}\n",
        dropped.len(),
        if dropped.is_empty() {
            "none".to_string()
        } else {
            dropped.join(", ")
        }
    ));

    // ---- model comparison ----
    out.push_str("\nModel comparison\n");
    out.push_str(&format!(
        "  {}{}{}{}\n",
        pad_right("model", 18),
        pad_left("predictors", 12),
        pad_left(crit.report_label(), num_w),
        pad_left("adjusted R²", num_w + 2)
    ));
    let report_crit = if crit == Criterion::PValue {
        Criterion::Aic
    } else {
        crit
    };
    let null_fit = fit_subset(&cx, &[]);
    let full_fit = fit_subset(&cx, &(0..k).collect::<Vec<_>>());
    let mut row = |label: &str, f: &Option<Fit>| {
        let (c, a, n) = match f {
            Some(f) => (
                fmt(f.info(report_crit, cx.n), d),
                fmt(f.adj_r2(cx.cyy, cx.n), d),
                f.subset.len().to_string(),
            ),
            None => ("n/a".into(), "n/a".into(), "-".into()),
        };
        out.push_str(&format!(
            "  {}{}{}{}\n",
            pad_right(label, 18),
            pad_left(&n, 12),
            pad_left(&c, num_w),
            pad_left(&a, num_w + 2)
        ));
    };
    row("intercept only", &null_fit);
    row("selected", &Some(fit.clone()));
    row("all predictors", &full_fit);
    if full_fit.is_none() {
        out.push_str(
            "  (the all-predictor model is collinear or has too few rows, so it cannot be fitted)\n",
        );
    }

    Ok(out)
}

impl Criterion {
    fn report_label(self) -> &'static str {
        match self {
            Criterion::PValue => "AIC",
            _ => self.label(),
        }
    }
}

/// Positional entry point shared by the chat block, the CLI and the web page.
/// Empty strings fall back to the documented defaults so a blank page field
/// behaves like an omitted chat argument.
#[allow(clippy::too_many_arguments)]
pub fn run(
    data: &str,
    target: &str,
    direction: &str,
    criterion: &str,
    alpha_enter: f64,
    alpha_remove: f64,
    force: &str,
    labels: &str,
    header: bool,
    decimals: u32,
) -> Result<String, String> {
    let d = Options::default();
    let or = |v: &str, fallback: &str| {
        if v.trim().is_empty() {
            fallback.to_string()
        } else {
            v.trim().to_string()
        }
    };
    select_report(&Options {
        data: data.to_string(),
        target: or(target, &d.target),
        direction: or(direction, &d.direction),
        criterion: or(criterion, &d.criterion),
        alpha_enter,
        alpha_remove,
        force: force.to_string(),
        labels: labels.to_string(),
        header,
        decimals,
    })
}

// ---------------------------------------------------------------------------
// tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// x1 drives y, x2 is noise: y = 2 + 3·x1 + e, where e was constructed to be
    /// orthogonal to x2, so x2 explains none of the leftover variance.
    const DATA: &str = "1,5,5.19\n2,3,7.61\n3,8,11.29\n4,1,13.85\n5,9,16.95\n6,2,20.4\n7,7,22.51\n8,4,26.33\n9,6,28.73\n10,10,32.14";

    /// Header-row dataset: sales = 60 + 4·ads − 2·price + e, with temp orthogonal
    /// noise. Mirrors the page's worked example.
    const MARKETING: &str = "ads,price,temp,sales\n2,19,21,30.4\n5,15,14,49.0\n3,18,30,37.0\n8,12,18,67.1\n6,14,25,56.4\n9,11,11,75.1\n4,17,27,40.4\n7,13,16,62.7\n10,10,23,79.7\n5,16,29,48.4\n8,12,13,67.8\n3,20,19,31.9";

    fn opts(data: &str) -> Options {
        Options {
            data: data.into(),
            ..Default::default()
        }
    }

    #[test]
    fn forward_aic_keeps_the_real_predictor_and_drops_the_noise() {
        let mut o = opts(DATA);
        o.direction = "forward".into();
        o.labels = "x1,x2,y".into();
        let r = select_report(&o).unwrap();
        assert!(r.contains("+ x1"), "x1 should enter:\n{r}");
        assert!(!r.contains("+ x2"), "x2 should not enter:\n{r}");
        assert!(r.contains("Dropped (1): x2"), "{r}");
        assert!(r.contains("Selected model (1 of 2 predictors)"), "{r}");
    }

    #[test]
    fn backward_elimination_reaches_the_same_model() {
        let mut o = opts(DATA);
        o.direction = "backward".into();
        o.labels = "x1,x2,y".into();
        let r = select_report(&o).unwrap();
        assert!(r.contains("- x2"), "x2 should be eliminated:\n{r}");
        assert!(r.contains("Dropped (1): x2"), "{r}");
    }

    #[test]
    fn bidirectional_is_the_default_and_agrees() {
        let r = select_report(&opts(DATA)).unwrap();
        assert!(r.contains("bidirectional stepwise"), "{r}");
        assert!(r.contains("Dropped (1): v2"), "{r}");
    }

    #[test]
    fn bic_and_aicc_are_selectable() {
        for c in ["bic", "aicc"] {
            let mut o = opts(DATA);
            o.criterion = c.into();
            let r = select_report(&o).unwrap();
            assert!(r.contains("Dropped (1): v2"), "{c}:\n{r}");
        }
    }

    #[test]
    fn pvalue_strategy_uses_the_alpha_thresholds() {
        let mut o = opts(DATA);
        o.criterion = "pvalue".into();
        o.direction = "forward".into();
        o.labels = "x1,x2,y".into();
        let r = select_report(&o).unwrap();
        assert!(r.contains("+ x1"), "{r}");
        assert!(r.contains("p-value"), "{r}");
        assert!(r.contains("Dropped (1): x2"), "{r}");

        // x2 is orthogonal noise (p ≈ 0.99), so only a near-1 alpha lets it in.
        o.alpha_enter = 0.999;
        o.alpha_remove = 0.9999;
        let loose = select_report(&o).unwrap();
        assert!(loose.contains("+ x2"), "{loose}");
        assert!(loose.contains("Dropped (0): none"), "{loose}");
    }

    #[test]
    fn force_keeps_a_predictor_that_would_be_dropped() {
        let mut o = opts(DATA);
        o.labels = "x1,x2,y".into();
        o.force = "x2".into();
        let r = select_report(&o).unwrap();
        assert!(r.contains("Forced in: x2"), "{r}");
        assert!(r.contains("Dropped (0): none"), "{r}");
    }

    #[test]
    fn header_row_names_the_columns_and_target_picks_by_name() {
        let mut o = opts(MARKETING);
        o.header = true;
        o.target = "sales".into();
        let r = select_report(&o).unwrap();
        assert!(r.contains("target: sales"), "{r}");
        assert!(r.contains("+ ads"), "{r}");
        assert!(r.contains("+ price"), "{r}");
        assert!(r.contains("Dropped (1): temp"), "{r}");
        assert!(r.contains("Selected model (2 of 3 predictors)"), "{r}");
    }

    #[test]
    fn target_accepts_a_one_based_index() {
        let mut o = opts(DATA);
        o.target = "3".into();
        let r = select_report(&o).unwrap();
        assert!(r.contains("target: v3"), "{r}");
    }

    #[test]
    fn intercept_only_wins_when_nothing_explains_the_target() {
        // y alternates independently of both predictors.
        let o = opts("1,2,1\n2,4,-1\n3,6,1\n4,8,-1\n5,10,1\n6,12,-1\n7,14,1\n8,16,-1");
        let r = select_report(&o).unwrap();
        assert!(r.contains("Selected model (0 of 2 predictors)"), "{r}");
        assert!(r.contains("intercept-only model wins"), "{r}");
    }

    #[test]
    fn decimals_control_the_rounding() {
        let mut o = opts(DATA);
        o.decimals = 1;
        let r = select_report(&o).unwrap();
        assert!(r.contains("R² 1.0"), "{r}");
        assert!(!r.contains("R² 1.0000"), "{r}");
    }

    #[test]
    fn non_numeric_cell_is_reported_with_its_position() {
        let e = select_report(&opts("1,2,3\n4,oops,6\n7,8,9")).unwrap_err();
        assert_eq!(e, "row 2, column 2: expected a number, got 'oops'");
    }

    #[test]
    fn ragged_rows_are_rejected() {
        let e = select_report(&opts("1,2,3\n4,5\n7,8,9")).unwrap_err();
        assert!(e.contains("row 2 has 2 values but the first data row has 3"), "{e}");
    }

    #[test]
    fn too_few_rows_is_rejected() {
        let e = select_report(&opts("1,2,3\n4,5,6")).unwrap_err();
        assert!(e.contains("at least 3 data rows"), "{e}");
    }

    #[test]
    fn single_column_is_rejected() {
        let e = select_report(&opts("1\n2\n3\n4")).unwrap_err();
        assert!(e.contains("at least 2 columns"), "{e}");
    }

    #[test]
    fn constant_target_is_rejected() {
        let e = select_report(&opts("1,2,7\n2,4,7\n3,9,7\n4,1,7")).unwrap_err();
        assert!(e.contains("is constant"), "{e}");
    }

    #[test]
    fn unknown_direction_and_criterion_are_rejected() {
        let mut o = opts(DATA);
        o.direction = "sideways".into();
        assert!(select_report(&o).unwrap_err().contains("direction must be"));
        let mut o = opts(DATA);
        o.criterion = "r2".into();
        assert!(select_report(&o).unwrap_err().contains("criterion must be"));
    }

    #[test]
    fn unknown_target_names_the_available_columns() {
        let mut o = opts(DATA);
        o.target = "nope".into();
        let e = select_report(&o).unwrap_err();
        assert!(e.contains("matches no column"), "{e}");
        assert!(e.contains("v1, v2, v3"), "{e}");
    }

    #[test]
    fn alpha_remove_below_alpha_enter_is_rejected() {
        let mut o = opts(DATA);
        o.criterion = "pvalue".into();
        o.alpha_enter = 0.10;
        o.alpha_remove = 0.05;
        assert!(select_report(&o)
            .unwrap_err()
            .contains("must be at least alpha_enter"));
    }

    /// Build a `rows × cols` table whose values vary per row and column.
    fn grid(rows: usize, cols: usize) -> String {
        (0..rows)
            .map(|r| {
                (0..cols)
                    .map(|c| ((r * 31 + c * 17) % 13).to_string())
                    .collect::<Vec<_>>()
                    .join(",")
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn column_cap_is_enforced_at_the_boundary() {
        // Exactly MAX_COLS columns is accepted…
        let mut at = opts(&grid(20, MAX_COLS));
        at.direction = "forward".into();
        let r = select_report(&at).unwrap();
        assert!(r.contains(&format!("candidate predictors: {}", MAX_COLS - 1)), "{r}");

        // …one more is rejected with the cap named.
        let over = opts(&grid(20, MAX_COLS + 1));
        let e = select_report(&over).unwrap_err();
        assert!(e.contains("too many columns: 61 (max 60)"), "{e}");
    }

    #[test]
    fn row_cap_is_enforced() {
        let over = opts(&grid(MAX_ROWS + 1, 3));
        let e = select_report(&over).unwrap_err();
        assert!(e.contains("too many rows: 20001 (max 20000)"), "{e}");
    }

    #[test]
    fn a_duplicated_column_is_skipped_rather_than_crashing() {
        // v2 is an exact copy of v1: the collinear pair can never be fitted
        // together, so the search still terminates with a usable model.
        let o = opts("1,1,5.1\n2,2,7.9\n3,3,11.2\n4,4,13.8\n5,5,17.1\n6,6,19.9");
        let r = select_report(&o).unwrap();
        assert!(r.contains("Selected model (1 of 2 predictors)"), "{r}");
    }

    #[test]
    fn run_wrapper_falls_back_to_defaults_on_blank_strings() {
        let r = run(DATA, "", "", "", 0.05, 0.10, "", "x1,x2,y", false, 4).unwrap();
        assert!(r.contains("bidirectional stepwise, AIC"), "{r}");
        assert!(r.contains("Dropped (1): x2"), "{r}");

        let e = run(DATA, "", "sideways", "", 0.05, 0.10, "", "", false, 4).unwrap_err();
        assert!(e.contains("direction must be"), "{e}");
    }

    #[test]
    fn separators_and_comments_are_flexible() {
        let tabbed = "# a comment row\n1\t5\t5.1\n2\t3\t7.9\n3\t8\t11.2\n4\t1\t13.8\n5 9 17.1\n6;2;19.9";
        let r = select_report(&opts(tabbed)).unwrap();
        assert!(r.contains("Observations: 6"), "{r}");
    }
}
