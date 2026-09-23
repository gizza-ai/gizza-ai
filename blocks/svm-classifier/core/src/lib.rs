//! svm-classifier core — pure compute, shared by the chat skill block and the web page.
//! No wafer/wasm-bindgen deps, no third-party crates.
//!
//! Trains a C-SVC support vector machine on a pasted table and reports what makes
//! an SVM worth reading: the margin, the support vectors, and the accuracy.
//!
//! The solver is a self-contained SMO (sequential minimal optimisation) with
//! second-order working-set selection — the same dual formulation the reference
//! C-SVC implementations use:
//!
//! ```text
//! min  ½ αᵀQα − eᵀα      subject to  yᵀα = 0,  0 ≤ αᵢ ≤ Cᵢ
//! ```
//!
//! with `Qᵢⱼ = yᵢ yⱼ K(xᵢ, xⱼ)`. Four kernels are available (linear, RBF,
//! polynomial, sigmoid). The margin is computed in feature space as `2 / ‖w‖`
//! where `‖w‖² = ΣᵢΣⱼ αᵢαⱼyᵢyⱼK(xᵢ,xⱼ)`, so it is reported for every kernel and
//! not only for the linear one.
//!
//! Everything is deterministic: the solver is exact given the data, and the only
//! randomness — the hold-out shuffle and the cross-validation fold assignment —
//! is driven by `seed`.

/// Hard caps — keep a pasted table inside what a browser tab can chew through.
/// The solver caches the full `n × n` kernel matrix, so `MAX_ROWS` is chosen so
/// that matrix stays around 32 MB.
pub const MAX_ROWS: usize = 2_000;
pub const MAX_COLS: usize = 100;
/// A categorical feature column with more distinct values than this is almost
/// certainly an id column, and one-hot encoding it would explode the matrix.
pub const MAX_LEVELS: usize = 50;
/// One-vs-one grows quadratically in the class count.
pub const MAX_CLASSES: usize = 20;
/// Cap on rows pasted into `predict`.
pub const MAX_PREDICT_ROWS: usize = 1_000;
/// Support vectors listed individually before the table is truncated.
pub const MAX_SV_LISTED: usize = 50;

const TAU: f64 = 1e-12;

/// Every knob the tool exposes. `Default` mirrors the descriptor defaults.
#[derive(Clone, Debug)]
pub struct Options {
    pub target: String,
    pub features: String,
    pub kernel: String,
    pub c: f64,
    pub gamma: String,
    pub degree: u32,
    pub coef0: f64,
    pub scaling: String,
    pub class_weight: String,
    pub multiclass: String,
    pub tol: f64,
    pub max_iter: u32,
    pub cv_folds: u32,
    pub test_split: f64,
    pub seed: u64,
    pub predict: String,
    pub header: String,
    pub decimals: u32,
    pub format: String,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            target: "last".into(),
            features: String::new(),
            kernel: "rbf".into(),
            c: 1.0,
            gamma: "scale".into(),
            degree: 3,
            coef0: 0.0,
            scaling: "standard".into(),
            class_weight: "none".into(),
            multiclass: "ovo".into(),
            tol: 0.001,
            max_iter: 100_000,
            cv_folds: 0,
            test_split: 0.0,
            seed: 42,
            predict: String::new(),
            header: "auto".into(),
            decimals: 4,
            format: "text".into(),
        }
    }
}

/// Deterministic xorshift64* — used only to shuffle rows for `test_split` and
/// `cv_folds`, so a `seed` reproduces a split exactly. Solving itself is exact.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        // SplitMix-style mixing so tiny seeds (0, 1, 2) still give distinct streams.
        let mut x = seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        x ^= x >> 33;
        x = x.wrapping_mul(0xff51_afd7_ed55_8ccd);
        x ^= x >> 33;
        Rng(x | 1)
    }
    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }
    fn shuffle(&mut self, v: &mut [usize]) {
        for i in (1..v.len()).rev() {
            let j = (self.next_u64() % (i as u64 + 1)) as usize;
            v.swap(i, j);
        }
    }
}

// ---------------------------------------------------------------- parsing ---

fn is_missing(tok: &str) -> bool {
    matches!(
        tok.trim().to_ascii_lowercase().as_str(),
        "" | "na" | "n/a" | "nan" | "null" | "none" | "-" | "?" | "."
    )
}

fn looks_numeric(tok: &str) -> bool {
    tok.trim().parse::<f64>().is_ok_and(|v| v.is_finite())
}

fn split_row(line: &str, delim: Option<char>) -> Vec<String> {
    match delim {
        Some(d) => line.split(d).map(|s| s.trim().to_string()).collect(),
        None => line.split_whitespace().map(|s| s.to_string()).collect(),
    }
}

/// Pick the column delimiter from the first non-blank line: whichever of
/// comma / tab / semicolon / pipe occurs most, else any run of whitespace.
fn detect_delim(line: &str) -> Option<char> {
    let mut best: Option<(char, usize)> = None;
    for d in [',', '\t', ';', '|'] {
        let n = line.matches(d).count();
        if n > 0 && best.map(|(_, b)| n > b).unwrap_or(true) {
            best = Some((d, n));
        }
    }
    best.map(|(d, _)| d)
}

fn grid(data: &str) -> Vec<Vec<String>> {
    let lines: Vec<&str> = data
        .lines()
        .map(|l| l.trim_end_matches('\r'))
        .filter(|l| !l.trim().is_empty())
        .collect();
    let Some(first) = lines.first() else {
        return Vec::new();
    };
    let delim = detect_delim(first);
    lines.iter().map(|l| split_row(l, delim)).collect()
}

struct Table {
    names: Vec<String>,
    rows: Vec<Vec<String>>,
}

fn parse_table(data: &str, header_mode: &str) -> Result<Table, String> {
    let mut rows = grid(data);
    if rows.is_empty() {
        return Err(
            "no data: paste a table with one row per observation, e.g. 'x,y,label' then '1,2,a'"
                .into(),
        );
    }
    let ncol = rows[0].len();
    if ncol < 2 {
        return Err(format!(
            "need at least 2 columns (at least one feature plus the class column), found {ncol}. Separate columns with commas, tabs, semicolons, pipes or spaces."
        ));
    }
    if ncol > MAX_COLS {
        return Err(format!("too many columns: {ncol} (max {MAX_COLS})"));
    }
    for (i, r) in rows.iter().enumerate() {
        if r.len() != ncol {
            return Err(format!(
                "row {} has {} columns but row 1 has {ncol} — every row must have the same number of columns",
                i + 1,
                r.len()
            ));
        }
    }

    let has_header = match header_mode.trim().to_ascii_lowercase().as_str() {
        "yes" | "true" => true,
        "no" | "false" => false,
        "" | "auto" => rows[0].iter().any(|t| !is_missing(t) && !looks_numeric(t)),
        other => {
            return Err(format!(
                "header must be 'auto', 'yes' or 'no' (got '{other}')"
            ))
        }
    };

    let names: Vec<String> = if has_header {
        let head = rows.remove(0);
        head.iter()
            .enumerate()
            .map(|(i, n)| {
                let n = n.trim();
                if n.is_empty() {
                    format!("c{}", i + 1)
                } else {
                    n.to_string()
                }
            })
            .collect()
    } else {
        (1..=ncol).map(|i| format!("c{i}")).collect()
    };

    if rows.is_empty() {
        return Err("no data rows: the table only has a header row".into());
    }
    if rows.len() > MAX_ROWS {
        return Err(format!(
            "too many rows: {} (max {MAX_ROWS}). The solver caches the full kernel matrix, so the row cap keeps memory bounded.",
            rows.len()
        ));
    }
    Ok(Table { names, rows })
}

/// Resolve a column selector: `last`, `first`, a 1-based index, or a column name.
fn resolve_column(sel: &str, names: &[String], what: &str) -> Result<usize, String> {
    let s = sel.trim();
    if s.is_empty() || s.eq_ignore_ascii_case("last") {
        return Ok(names.len() - 1);
    }
    if s.eq_ignore_ascii_case("first") {
        return Ok(0);
    }
    if let Ok(i) = s.parse::<usize>() {
        if i >= 1 && i <= names.len() {
            return Ok(i - 1);
        }
        return Err(format!(
            "{what} column index {i} is out of range (the table has {} columns)",
            names.len()
        ));
    }
    if let Some(i) = names.iter().position(|n| n.eq_ignore_ascii_case(s)) {
        return Ok(i);
    }
    Err(format!(
        "{what} column '{s}' not found — available columns: {}",
        names.join(", ")
    ))
}

// --------------------------------------------------------------- encoding ---

/// How one source column becomes one or more model columns.
enum Encoding {
    /// Numeric passthrough.
    Numeric,
    /// One-hot: one model column per level, in sorted order.
    OneHot(Vec<String>),
}

struct Design {
    /// Model-column names, e.g. `hours` or `color=red`.
    col_names: Vec<String>,
    /// `rows[i][j]` — the encoded (unscaled) design matrix.
    rows: Vec<Vec<f64>>,
    /// Class label index per row.
    y: Vec<usize>,
    /// Distinct class labels, sorted.
    classes: Vec<String>,
    /// Source feature columns, with their encodings, in order.
    feats: Vec<(usize, Encoding)>,
    n_numeric: usize,
    n_onehot: usize,
    dropped: usize,
}

/// Sort labels numerically when every one parses as a number, else lexically.
fn sort_labels(mut v: Vec<String>) -> Vec<String> {
    if v.iter().all(|s| looks_numeric(s)) {
        v.sort_by(|a, b| {
            let (x, y) = (a.trim().parse::<f64>().unwrap(), b.trim().parse::<f64>().unwrap());
            x.partial_cmp(&y).unwrap().then_with(|| a.cmp(b))
        });
    } else {
        v.sort();
    }
    v
}

fn distinct_sorted(values: impl Iterator<Item = String>) -> Vec<String> {
    let mut seen: Vec<String> = Vec::new();
    for v in values {
        if !seen.iter().any(|s| *s == v) {
            seen.push(v);
        }
    }
    sort_labels(seen)
}

fn build_design(t: &Table, opts: &Options) -> Result<Design, String> {
    let target = resolve_column(&opts.target, &t.names, "target")?;

    // Feature selection: explicit list, else every non-target column.
    let feat_cols: Vec<usize> = if opts.features.trim().is_empty() {
        (0..t.names.len()).filter(|i| *i != target).collect()
    } else {
        let mut out = Vec::new();
        for sel in opts.features.split(',') {
            let sel = sel.trim();
            if sel.is_empty() {
                continue;
            }
            let i = resolve_column(sel, &t.names, "feature")?;
            if i == target {
                return Err(format!(
                    "feature column '{}' is also the target column — pick a different target",
                    t.names[i]
                ));
            }
            if !out.contains(&i) {
                out.push(i);
            }
        }
        if out.is_empty() {
            return Err("features listed no usable columns — leave it empty to use every non-target column".into());
        }
        out
    };
    if feat_cols.is_empty() {
        return Err("no feature columns: the table needs at least one column besides the target".into());
    }

    // Keep only rows whose target and every selected feature are present.
    let keep: Vec<usize> = (0..t.rows.len())
        .filter(|&r| {
            !is_missing(&t.rows[r][target]) && feat_cols.iter().all(|&c| !is_missing(&t.rows[r][c]))
        })
        .collect();
    let dropped = t.rows.len() - keep.len();
    if keep.is_empty() {
        return Err(
            "every row has a missing value in the target or a selected feature — nothing left to train on"
                .into(),
        );
    }

    let classes = distinct_sorted(keep.iter().map(|&r| t.rows[r][target].trim().to_string()));
    if classes.len() < 2 {
        return Err(format!(
            "the target column has only one class ('{}') — a classifier needs at least two",
            classes[0]
        ));
    }
    if classes.len() > MAX_CLASSES {
        return Err(format!(
            "too many classes: {} (max {MAX_CLASSES}). '{}' looks like an id or a continuous column, not a class label.",
            classes.len(),
            t.names[target]
        ));
    }
    if classes.len() > keep.len() / 2 {
        return Err(format!(
            "{} classes across only {} usable rows — every class needs at least two rows to be separable",
            classes.len(),
            keep.len()
        ));
    }

    // Decide each feature's encoding from the kept rows.
    let mut feats: Vec<(usize, Encoding)> = Vec::new();
    let mut col_names: Vec<String> = Vec::new();
    let (mut n_numeric, mut n_onehot) = (0usize, 0usize);
    for &c in &feat_cols {
        let all_num = keep.iter().all(|&r| looks_numeric(&t.rows[r][c]));
        if all_num {
            feats.push((c, Encoding::Numeric));
            col_names.push(t.names[c].clone());
            n_numeric += 1;
        } else {
            let levels = distinct_sorted(keep.iter().map(|&r| t.rows[r][c].trim().to_string()));
            if levels.len() > MAX_LEVELS {
                return Err(format!(
                    "feature column '{}' has {} distinct values (max {MAX_LEVELS} for one-hot encoding) — it looks like an id column, so drop it with the features list",
                    t.names[c],
                    levels.len()
                ));
            }
            for l in &levels {
                col_names.push(format!("{}={}", t.names[c], l));
            }
            n_onehot += levels.len();
            feats.push((c, Encoding::OneHot(levels)));
        }
    }
    if col_names.len() > MAX_COLS * 4 {
        return Err(format!(
            "one-hot encoding produced {} model columns (max {}) — reduce the categorical columns with the features list",
            col_names.len(),
            MAX_COLS * 4
        ));
    }

    let mut rows = Vec::with_capacity(keep.len());
    let mut y = Vec::with_capacity(keep.len());
    for &r in &keep {
        rows.push(encode_row(&t.rows[r], &feats)?);
        let lab = t.rows[r][target].trim();
        y.push(classes.iter().position(|c| c == lab).unwrap());
    }

    Ok(Design {
        col_names,
        rows,
        y,
        classes,
        feats,
        n_numeric,
        n_onehot,
        dropped,
    })
}

/// Encode one raw row with the design's encodings. An unseen category encodes as
/// all-zeros for that column group (the same thing every one-hot encoder does).
fn encode_row(raw: &[String], feats: &[(usize, Encoding)]) -> Result<Vec<f64>, String> {
    let mut v = Vec::new();
    for (c, enc) in feats {
        match enc {
            Encoding::Numeric => {
                let tok = raw[*c].trim();
                let x: f64 = tok
                    .parse()
                    .map_err(|_| format!("'{tok}' is not a number in a numeric feature column"))?;
                if !x.is_finite() {
                    return Err(format!("'{tok}' is not a finite number"));
                }
                v.push(x);
            }
            Encoding::OneHot(levels) => {
                let tok = raw[*c].trim();
                for l in levels {
                    v.push(if l == tok { 1.0 } else { 0.0 });
                }
            }
        }
    }
    Ok(v)
}

// ---------------------------------------------------------------- scaling ---

/// Per-column affine scaling: `(x - shift) / scale`.
struct Scaler {
    shift: Vec<f64>,
    scale: Vec<f64>,
    label: &'static str,
}

impl Scaler {
    fn fit(rows: &[Vec<f64>], mode: &str) -> Result<Scaler, String> {
        let d = rows[0].len();
        let n = rows.len() as f64;
        match mode.trim().to_ascii_lowercase().as_str() {
            "none" => Ok(Scaler {
                shift: vec![0.0; d],
                scale: vec![1.0; d],
                label: "none (raw values)",
            }),
            "" | "standard" => {
                let mut shift = vec![0.0; d];
                let mut scale = vec![1.0; d];
                for j in 0..d {
                    let mean = rows.iter().map(|r| r[j]).sum::<f64>() / n;
                    let var = rows.iter().map(|r| (r[j] - mean).powi(2)).sum::<f64>() / n;
                    shift[j] = mean;
                    // A constant column has zero spread; leave it at its centred 0.
                    scale[j] = if var.sqrt() > 1e-12 { var.sqrt() } else { 1.0 };
                }
                Ok(Scaler {
                    shift,
                    scale,
                    label: "standard (z-score per column)",
                })
            }
            "minmax" => {
                let mut shift = vec![0.0; d];
                let mut scale = vec![1.0; d];
                for j in 0..d {
                    let lo = rows.iter().map(|r| r[j]).fold(f64::INFINITY, f64::min);
                    let hi = rows.iter().map(|r| r[j]).fold(f64::NEG_INFINITY, f64::max);
                    shift[j] = lo;
                    scale[j] = if hi - lo > 1e-12 { hi - lo } else { 1.0 };
                }
                Ok(Scaler {
                    shift,
                    scale,
                    label: "minmax (each column to 0–1)",
                })
            }
            other => Err(format!(
                "scaling must be 'standard', 'minmax' or 'none' (got '{other}')"
            )),
        }
    }

    fn apply(&self, row: &[f64]) -> Vec<f64> {
        row.iter()
            .enumerate()
            .map(|(j, x)| (x - self.shift[j]) / self.scale[j])
            .collect()
    }
}

// ----------------------------------------------------------------- kernel ---

#[derive(Clone, Copy, PartialEq)]
enum Kernel {
    Linear,
    Rbf,
    Poly,
    Sigmoid,
}

impl Kernel {
    fn parse(s: &str) -> Result<Kernel, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "rbf" | "gaussian" => Ok(Kernel::Rbf),
            "linear" => Ok(Kernel::Linear),
            "poly" | "polynomial" => Ok(Kernel::Poly),
            "sigmoid" | "tanh" => Ok(Kernel::Sigmoid),
            other => Err(format!(
                "kernel must be 'linear', 'rbf', 'poly' or 'sigmoid' (got '{other}')"
            )),
        }
    }
    fn label(self) -> &'static str {
        match self {
            Kernel::Linear => "linear",
            Kernel::Rbf => "RBF",
            Kernel::Poly => "polynomial",
            Kernel::Sigmoid => "sigmoid",
        }
    }
    /// Does this kernel actually consume `gamma`?
    fn uses_gamma(self) -> bool {
        !matches!(self, Kernel::Linear)
    }
}

struct KernelSpec {
    kind: Kernel,
    gamma: f64,
    degree: i32,
    coef0: f64,
}

impl KernelSpec {
    fn eval(&self, a: &[f64], b: &[f64]) -> f64 {
        match self.kind {
            Kernel::Linear => dot(a, b),
            Kernel::Rbf => {
                let mut s = 0.0;
                for (x, y) in a.iter().zip(b) {
                    let d = x - y;
                    s += d * d;
                }
                (-self.gamma * s).exp()
            }
            Kernel::Poly => (self.gamma * dot(a, b) + self.coef0).powi(self.degree),
            Kernel::Sigmoid => (self.gamma * dot(a, b) + self.coef0).tanh(),
        }
    }
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// Resolve `gamma`: `scale` = 1/(n_features · var(X)), `auto` = 1/n_features, or
/// an explicit positive number. Mirrors the reference libraries' two keywords.
fn resolve_gamma(spec: &str, rows: &[Vec<f64>]) -> Result<(f64, String), String> {
    let s = spec.trim().to_ascii_lowercase();
    let d = rows[0].len() as f64;
    match s.as_str() {
        "" | "scale" => {
            let n = (rows.len() * rows[0].len()) as f64;
            let mean = rows.iter().flat_map(|r| r.iter()).sum::<f64>() / n;
            let var = rows
                .iter()
                .flat_map(|r| r.iter())
                .map(|x| (x - mean).powi(2))
                .sum::<f64>()
                / n;
            let g = if var > 1e-12 { 1.0 / (d * var) } else { 1.0 / d };
            Ok((g, "scale".into()))
        }
        "auto" => Ok((1.0 / d, "auto".into())),
        _ => {
            let g: f64 = s
                .parse()
                .map_err(|_| format!("gamma must be 'scale', 'auto' or a positive number (got '{spec}')"))?;
            if !(g.is_finite() && g > 0.0) {
                return Err(format!("gamma must be a positive number (got '{spec}')"));
            }
            Ok((g, "explicit".into()))
        }
    }
}

/// Full cached kernel matrix over the encoded+scaled rows.
struct Gram {
    n: usize,
    k: Vec<f64>,
}

impl Gram {
    fn build(x: &[Vec<f64>], spec: &KernelSpec) -> Gram {
        let n = x.len();
        let mut k = vec![0.0; n * n];
        for i in 0..n {
            for j in i..n {
                let v = spec.eval(&x[i], &x[j]);
                k[i * n + j] = v;
                k[j * n + i] = v;
            }
        }
        Gram { n, k }
    }
    #[inline]
    fn at(&self, i: usize, j: usize) -> f64 {
        self.k[i * self.n + j]
    }
}

// ------------------------------------------------------------- SMO solver ---

/// One trained binary sub-model. `sv` indexes the global encoded row list.
struct BinaryModel {
    /// Global row index of each support vector.
    sv: Vec<usize>,
    /// αᵢ · yᵢ for each support vector — the dual coefficients.
    coef: Vec<f64>,
    alpha: Vec<f64>,
    /// True when the support vector sits at the box bound `αᵢ = Cᵢ`.
    bounded: Vec<bool>,
    /// yᵢ ∈ {+1, −1} per support vector.
    sy: Vec<f64>,
    rho: f64,
    w_norm: f64,
    iters: u32,
    converged: bool,
    /// Class indices this sub-model separates: positive vs negative.
    pos: usize,
    neg: usize,
    n_sv_pos: usize,
    n_sv_neg: usize,
}

impl BinaryModel {
    fn margin(&self) -> f64 {
        if self.w_norm > 1e-12 {
            2.0 / self.w_norm
        } else {
            f64::INFINITY
        }
    }
    /// Decision value for an arbitrary encoded+scaled vector.
    fn decide_vec(&self, x: &[f64], xs: &[Vec<f64>], spec: &KernelSpec) -> f64 {
        let mut s = 0.0;
        for (k, &i) in self.sv.iter().enumerate() {
            s += self.coef[k] * spec.eval(&xs[i], x);
        }
        s - self.rho
    }
    /// Decision value for a row already in the kernel cache.
    fn decide_cached(&self, row: usize, g: &Gram) -> f64 {
        let mut s = 0.0;
        for (k, &i) in self.sv.iter().enumerate() {
            s += self.coef[k] * g.at(i, row);
        }
        s - self.rho
    }
}

/// Solve one binary C-SVC sub-problem over `idx` (global row indices) with
/// labels `y` (+1/−1) and per-side costs.
#[allow(clippy::too_many_arguments)]
fn solve_binary(
    idx: &[usize],
    y: &[f64],
    c_pos: f64,
    c_neg: f64,
    g: &Gram,
    tol: f64,
    max_iter: u32,
    pos: usize,
    neg: usize,
) -> BinaryModel {
    let m = idx.len();
    let cost = |t: usize| if y[t] > 0.0 { c_pos } else { c_neg };
    let kl = |a: usize, b: usize| g.at(idx[a], idx[b]);

    let mut alpha = vec![0.0f64; m];
    // Gᵢ = Σⱼ Qᵢⱼ αⱼ − 1; with α = 0 that is −1 everywhere.
    let mut grad = vec![-1.0f64; m];
    let mut qi = vec![0.0f64; m];
    let mut qj = vec![0.0f64; m];
    let mut iters = 0u32;
    let mut converged = false;

    while iters < max_iter {
        // --- working-set selection, first index: the maximal violating pair's
        //     "up" side, i = argmax over I_up of −yₜGₜ.
        let mut gmax = f64::NEG_INFINITY;
        let mut i = usize::MAX;
        for t in 0..m {
            let in_up = (y[t] > 0.0 && alpha[t] < cost(t)) || (y[t] < 0.0 && alpha[t] > 0.0);
            if in_up {
                let v = -y[t] * grad[t];
                if v > gmax {
                    gmax = v;
                    i = t;
                }
            }
        }
        if i == usize::MAX {
            converged = true;
            break;
        }
        for t in 0..m {
            qi[t] = y[i] * y[t] * kl(i, t);
        }

        // --- second index by second-order gain, plus the stopping quantity.
        let mut gmax2 = f64::NEG_INFINITY;
        let mut best_obj = f64::INFINITY;
        let mut j = usize::MAX;
        for t in 0..m {
            let in_low = (y[t] > 0.0 && alpha[t] > 0.0) || (y[t] < 0.0 && alpha[t] < cost(t));
            if !in_low {
                continue;
            }
            let gt = y[t] * grad[t];
            if gt > gmax2 {
                gmax2 = gt;
            }
            let b = gmax + gt;
            if b > 0.0 {
                // Qᵢᵢ + Qₜₜ − 2Qᵢₜyᵢyₜyᵢyₜ  =  Kᵢᵢ + Kₜₜ − 2Kᵢₜ  ≥ 0
                let mut quad = kl(i, i) + kl(t, t) - 2.0 * y[i] * y[t] * qi[t];
                if quad <= 0.0 {
                    quad = TAU;
                }
                let obj = -(b * b) / quad;
                if obj < best_obj {
                    best_obj = obj;
                    j = t;
                }
            }
        }
        // KKT violation is below tolerance → optimal.
        if gmax + gmax2 < tol || j == usize::MAX {
            converged = true;
            break;
        }

        for t in 0..m {
            qj[t] = y[j] * y[t] * kl(j, t);
        }

        // --- analytic two-variable update, clipped to the box.
        let (old_i, old_j) = (alpha[i], alpha[j]);
        let (ci, cj) = (cost(i), cost(j));
        if y[i] != y[j] {
            // qi[j] = Qᵢⱼ = yᵢyⱼKᵢⱼ already carries the label signs, and
            // yᵢ ≠ yⱼ ⇒ qi[j] = −Kᵢⱼ, so this is Kᵢᵢ + Kⱼⱼ − 2Kᵢⱼ.
            let mut quad = kl(i, i) + kl(j, j) + 2.0 * qi[j];
            if quad <= 0.0 {
                quad = TAU;
            }
            let delta = (-grad[i] - grad[j]) / quad;
            let diff = alpha[i] - alpha[j];
            alpha[i] += delta;
            alpha[j] += delta;
            if diff > 0.0 {
                if alpha[j] < 0.0 {
                    alpha[j] = 0.0;
                    alpha[i] = diff;
                }
            } else if alpha[i] < 0.0 {
                alpha[i] = 0.0;
                alpha[j] = -diff;
            }
            if diff > ci - cj {
                if alpha[i] > ci {
                    alpha[i] = ci;
                    alpha[j] = ci - diff;
                }
            } else if alpha[j] > cj {
                alpha[j] = cj;
                alpha[i] = cj + diff;
            }
        } else {
            // yᵢ = yⱼ ⇒ qi[j] = +Kᵢⱼ, so this is likewise Kᵢᵢ + Kⱼⱼ − 2Kᵢⱼ.
            let mut quad = kl(i, i) + kl(j, j) - 2.0 * qi[j];
            if quad <= 0.0 {
                quad = TAU;
            }
            let delta = (grad[i] - grad[j]) / quad;
            let sum = alpha[i] + alpha[j];
            alpha[i] -= delta;
            alpha[j] += delta;
            if sum > ci {
                if alpha[i] > ci {
                    alpha[i] = ci;
                    alpha[j] = sum - ci;
                }
            } else if alpha[j] < 0.0 {
                alpha[j] = 0.0;
                alpha[i] = sum;
            }
            if sum > cj {
                if alpha[j] > cj {
                    alpha[j] = cj;
                    alpha[i] = sum - cj;
                }
            } else if alpha[i] < 0.0 {
                alpha[i] = 0.0;
                alpha[j] = sum;
            }
        }

        let (di, dj) = (alpha[i] - old_i, alpha[j] - old_j);
        for t in 0..m {
            grad[t] += qi[t] * di + qj[t] * dj;
        }
        iters += 1;
    }

    // --- bias (ρ): average over free support vectors, else the box midpoint.
    let (mut n_free, mut sum_free) = (0usize, 0.0f64);
    let (mut ub, mut lb) = (f64::INFINITY, f64::NEG_INFINITY);
    for t in 0..m {
        let yg = y[t] * grad[t];
        if alpha[t] >= cost(t) {
            if y[t] < 0.0 {
                ub = ub.min(yg);
            } else {
                lb = lb.max(yg);
            }
        } else if alpha[t] <= 0.0 {
            if y[t] > 0.0 {
                ub = ub.min(yg);
            } else {
                lb = lb.max(yg);
            }
        } else {
            n_free += 1;
            sum_free += yg;
        }
    }
    let rho = if n_free > 0 {
        sum_free / n_free as f64
    } else if ub.is_finite() && lb.is_finite() {
        (ub + lb) / 2.0
    } else {
        0.0
    };

    // ‖w‖² = ΣᵢΣⱼ αᵢαⱼQᵢⱼ = Σᵢ αᵢ(Gᵢ + 1)
    let w_sq: f64 = (0..m).map(|t| alpha[t] * (grad[t] + 1.0)).sum();
    let w_norm = if w_sq > 0.0 { w_sq.sqrt() } else { 0.0 };

    let (mut sv, mut coef, mut al, mut bounded, mut sy) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let (mut n_sv_pos, mut n_sv_neg) = (0usize, 0usize);
    for t in 0..m {
        if alpha[t] > 0.0 {
            sv.push(idx[t]);
            coef.push(alpha[t] * y[t]);
            al.push(alpha[t]);
            bounded.push(alpha[t] >= cost(t) - 1e-12);
            sy.push(y[t]);
            if y[t] > 0.0 {
                n_sv_pos += 1;
            } else {
                n_sv_neg += 1;
            }
        }
    }

    BinaryModel {
        sv,
        coef,
        alpha: al,
        bounded,
        sy,
        rho,
        w_norm,
        iters,
        converged,
        pos,
        neg,
        n_sv_pos,
        n_sv_neg,
    }
}

// ------------------------------------------------------------ multi-class ---

struct Svm {
    models: Vec<BinaryModel>,
    /// `ovo`, `ovr`, or `binary` when there are exactly two classes.
    strategy: &'static str,
    n_classes: usize,
}

impl Svm {
    /// Predict a class index from a set of per-sub-model decision values.
    fn vote(&self, dec: &[f64]) -> (usize, Vec<u32>) {
        match self.strategy {
            "ovr" => {
                let mut best = 0usize;
                let mut best_v = f64::NEG_INFINITY;
                for (m, d) in self.models.iter().zip(dec) {
                    if *d > best_v {
                        best_v = *d;
                        best = m.pos;
                    }
                }
                (best, Vec::new())
            }
            _ => {
                let mut votes = vec![0u32; self.n_classes];
                for (m, d) in self.models.iter().zip(dec) {
                    if *d > 0.0 {
                        votes[m.pos] += 1;
                    } else {
                        votes[m.neg] += 1;
                    }
                }
                // Ties go to the lowest class index, which keeps output stable.
                let mut best = 0usize;
                for c in 1..self.n_classes {
                    if votes[c] > votes[best] {
                        best = c;
                    }
                }
                (best, votes)
            }
        }
    }

    fn predict_cached(&self, row: usize, g: &Gram) -> usize {
        let dec: Vec<f64> = self.models.iter().map(|m| m.decide_cached(row, g)).collect();
        self.vote(&dec).0
    }

    fn predict_vec(&self, x: &[f64], xs: &[Vec<f64>], spec: &KernelSpec) -> (usize, f64, Vec<u32>) {
        let dec: Vec<f64> = self
            .models
            .iter()
            .map(|m| m.decide_vec(x, xs, spec))
            .collect();
        let (cls, votes) = self.vote(&dec);
        // With a single sub-model the decision value is the signed distance the
        // prediction is based on; with several it is the winning pair's value.
        let head = if dec.len() == 1 {
            dec[0]
        } else {
            let mut acc = 0.0;
            for (m, d) in self.models.iter().zip(&dec) {
                if m.pos == cls {
                    acc += *d;
                } else if m.neg == cls {
                    acc -= *d;
                }
            }
            acc
        };
        (cls, head, votes)
    }
}

/// Per-class cost multipliers. `balanced` mirrors the standard
/// `n_samples / (n_classes · count(class))` weighting.
fn class_costs(y: &[usize], n_classes: usize, c: f64, mode: &str) -> Result<Vec<f64>, String> {
    match mode.trim().to_ascii_lowercase().as_str() {
        "" | "none" => Ok(vec![c; n_classes]),
        "balanced" => {
            let n = y.len() as f64;
            let mut counts = vec![0usize; n_classes];
            for &v in y {
                counts[v] += 1;
            }
            Ok(counts
                .iter()
                .map(|&k| {
                    if k == 0 {
                        c
                    } else {
                        c * n / (n_classes as f64 * k as f64)
                    }
                })
                .collect())
        }
        other => Err(format!(
            "class_weight must be 'none' or 'balanced' (got '{other}')"
        )),
    }
}

/// Train the full (possibly multi-class) model over `idx`.
#[allow(clippy::too_many_arguments)]
fn train(
    idx: &[usize],
    y: &[usize],
    n_classes: usize,
    costs: &[f64],
    g: &Gram,
    strategy: &str,
    tol: f64,
    max_iter: u32,
) -> Result<Svm, String> {
    let mut models = Vec::new();
    let strategy: &'static str = if n_classes == 2 {
        "binary"
    } else if strategy.trim().eq_ignore_ascii_case("ovr") {
        "ovr"
    } else {
        "ovo"
    };

    match strategy {
        "binary" => {
            let ys: Vec<f64> = idx.iter().map(|&r| if y[r] == 1 { 1.0 } else { -1.0 }).collect();
            models.push(solve_binary(
                idx, &ys, costs[1], costs[0], g, tol, max_iter, 1, 0,
            ));
        }
        "ovr" => {
            for c in 0..n_classes {
                let ys: Vec<f64> = idx.iter().map(|&r| if y[r] == c { 1.0 } else { -1.0 }).collect();
                // The "rest" side pools every other class; use the mean of their costs.
                let rest: f64 = (0..n_classes).filter(|k| *k != c).map(|k| costs[k]).sum::<f64>()
                    / (n_classes - 1) as f64;
                models.push(solve_binary(idx, &ys, costs[c], rest, g, tol, max_iter, c, usize::MAX));
            }
        }
        _ => {
            for a in 0..n_classes {
                for b in (a + 1)..n_classes {
                    let sub: Vec<usize> = idx
                        .iter()
                        .copied()
                        .filter(|&r| y[r] == a || y[r] == b)
                        .collect();
                    if sub.is_empty() {
                        continue;
                    }
                    // Positive = the higher class index, so `decision > 0` reads
                    // consistently with the binary case's "second class wins".
                    let ys: Vec<f64> = sub.iter().map(|&r| if y[r] == b { 1.0 } else { -1.0 }).collect();
                    models.push(solve_binary(
                        &sub, &ys, costs[b], costs[a], g, tol, max_iter, b, a,
                    ));
                }
            }
        }
    }
    if models.is_empty() {
        return Err("no sub-models could be trained — check the class column".into());
    }
    Ok(Svm {
        models,
        strategy,
        n_classes,
    })
}

// ---------------------------------------------------------------- metrics ---

struct Metrics {
    correct: usize,
    total: usize,
    /// `cm[actual][predicted]`
    cm: Vec<Vec<usize>>,
}

impl Metrics {
    fn accuracy(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            self.correct as f64 / self.total as f64
        }
    }
    /// (precision, recall, f1, support) per class.
    fn per_class(&self) -> Vec<(f64, f64, f64, usize)> {
        let k = self.cm.len();
        (0..k)
            .map(|c| {
                let tp = self.cm[c][c] as f64;
                let pred: f64 = (0..k).map(|a| self.cm[a][c] as f64).sum();
                let act: f64 = (0..k).map(|p| self.cm[c][p] as f64).sum();
                let prec = if pred > 0.0 { tp / pred } else { 0.0 };
                let rec = if act > 0.0 { tp / act } else { 0.0 };
                let f1 = if prec + rec > 0.0 {
                    2.0 * prec * rec / (prec + rec)
                } else {
                    0.0
                };
                (prec, rec, f1, act as usize)
            })
            .collect()
    }
    fn macro_avg(&self) -> (f64, f64, f64) {
        let pc = self.per_class();
        let k = pc.len() as f64;
        (
            pc.iter().map(|p| p.0).sum::<f64>() / k,
            pc.iter().map(|p| p.1).sum::<f64>() / k,
            pc.iter().map(|p| p.2).sum::<f64>() / k,
        )
    }
}

fn evaluate(model: &Svm, idx: &[usize], y: &[usize], n_classes: usize, g: &Gram) -> Metrics {
    let mut cm = vec![vec![0usize; n_classes]; n_classes];
    let mut correct = 0;
    for &r in idx {
        let p = model.predict_cached(r, g);
        cm[y[r]][p] += 1;
        if p == y[r] {
            correct += 1;
        }
    }
    Metrics {
        correct,
        total: idx.len(),
        cm,
    }
}

/// Stratified split: take `frac` of every class into the hold-out set so both
/// sides keep every label. Deterministic given `seed`.
fn stratified_split(
    y: &[usize],
    idx: &[usize],
    n_classes: usize,
    frac: f64,
    seed: u64,
) -> (Vec<usize>, Vec<usize>) {
    let mut rng = Rng::new(seed);
    let (mut train, mut test) = (Vec::new(), Vec::new());
    for c in 0..n_classes {
        let mut members: Vec<usize> = idx.iter().copied().filter(|&r| y[r] == c).collect();
        rng.shuffle(&mut members);
        let n_test = ((members.len() as f64) * frac).floor() as usize;
        // Never strand a class with fewer than two training rows.
        let n_test = n_test.min(members.len().saturating_sub(2));
        test.extend_from_slice(&members[..n_test]);
        train.extend_from_slice(&members[n_test..]);
    }
    train.sort_unstable();
    test.sort_unstable();
    (train, test)
}

/// Stratified k-fold assignment. Returns the fold index per position in `idx`.
fn stratified_folds(y: &[usize], idx: &[usize], n_classes: usize, k: usize, seed: u64) -> Vec<Vec<usize>> {
    let mut rng = Rng::new(seed ^ 0x9E37_79B9_7F4A_7C15);
    let mut folds = vec![Vec::new(); k];
    for c in 0..n_classes {
        let mut members: Vec<usize> = idx.iter().copied().filter(|&r| y[r] == c).collect();
        rng.shuffle(&mut members);
        for (i, r) in members.into_iter().enumerate() {
            folds[i % k].push(r);
        }
    }
    for f in &mut folds {
        f.sort_unstable();
    }
    folds
}

// --------------------------------------------------------------- reporting --

fn fmt(v: f64, d: u32) -> String {
    if v.is_infinite() {
        return if v > 0.0 { "inf".into() } else { "-inf".into() };
    }
    if v.is_nan() {
        return "n/a".into();
    }
    let s = format!("{:.*}", d as usize, v);
    // Avoid "-0.0000" — it reads like a distinct value but is not one.
    if s.starts_with('-') && s[1..].chars().all(|c| c == '0' || c == '.') {
        s[1..].to_string()
    } else {
        s
    }
}

fn pct(v: f64) -> String {
    format!("{:.2}%", v * 100.0)
}

fn json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn json_num(v: f64, d: u32) -> String {
    if v.is_finite() {
        fmt(v, d)
    } else {
        "null".into()
    }
}

fn csv_cell(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// Right-pad to `w` display columns.
fn pad(s: &str, w: usize) -> String {
    let n = s.chars().count();
    if n >= w {
        s.to_string()
    } else {
        format!("{s}{}", " ".repeat(w - n))
    }
}

/// Left-pad to `w` display columns.
fn rpad(s: &str, w: usize) -> String {
    let n = s.chars().count();
    if n >= w {
        s.to_string()
    } else {
        format!("{}{s}", " ".repeat(w - n))
    }
}

// -------------------------------------------------------------- the report --

struct Prediction {
    row: usize,
    class: usize,
    decision: f64,
    votes: Vec<u32>,
}

struct Report<'a> {
    d: &'a Design,
    opts: &'a Options,
    spec: &'a KernelSpec,
    gamma_src: String,
    scaler_label: &'a str,
    model: &'a Svm,
    train_idx: Vec<usize>,
    train_metrics: Metrics,
    test_metrics: Option<Metrics>,
    cv: Option<(usize, f64, Vec<f64>)>,
    /// Linear-kernel weight vector, when there is exactly one sub-model.
    weights: Option<Vec<f64>>,
    /// Union of support-vector row indices across sub-models.
    sv_rows: Vec<usize>,
    /// Per-class support-vector counts.
    sv_by_class: Vec<usize>,
    predictions: Vec<Prediction>,
    notes: Vec<String>,
}

impl Report<'_> {
    fn headline_model(&self) -> &BinaryModel {
        &self.model.models[0]
    }

    fn text(&self) -> String {
        let d = self.opts.decimals;
        let mut o = String::new();
        let n_sub = self.model.models.len();

        o.push_str(&format!(
            "Support vector machine — {} kernel (C-SVC)\n",
            self.spec.kind.label()
        ));

        // --- data
        o.push_str("\nData\n");
        o.push_str(&format!(
            "  Rows used:        {} of {}\n",
            self.train_idx.len() + self.test_metrics.as_ref().map(|m| m.total).unwrap_or(0),
            self.d.rows.len() + self.d.dropped
        ));
        o.push_str(&format!(
            "  Features:         {} source → {} model columns ({} numeric, {} one-hot)\n",
            self.d.feats.len(),
            self.d.col_names.len(),
            self.d.n_numeric,
            self.d.n_onehot
        ));
        o.push_str(&format!(
            "  Classes:          {} ({})\n",
            self.d.classes.len(),
            self.d.classes.join(", ")
        ));
        o.push_str(&format!("  Scaling:          {}\n", self.scaler_label));
        o.push_str(&format!("  C (cost):         {}\n", fmt(self.opts.c, d)));
        if self.spec.kind.uses_gamma() {
            o.push_str(&format!(
                "  gamma:            {} → {}\n",
                self.gamma_src,
                fmt(self.spec.gamma, d.max(4))
            ));
        }
        if self.spec.kind == Kernel::Poly {
            o.push_str(&format!("  degree:           {}\n", self.spec.degree));
        }
        if matches!(self.spec.kind, Kernel::Poly | Kernel::Sigmoid) {
            o.push_str(&format!("  coef0:            {}\n", fmt(self.spec.coef0, d)));
        }
        if self.model.strategy != "binary" {
            o.push_str(&format!(
                "  Multiclass:       {} ({} sub-models)\n",
                if self.model.strategy == "ovr" {
                    "one-vs-rest"
                } else {
                    "one-vs-one"
                },
                n_sub
            ));
        }

        // --- model
        o.push_str("\nModel\n");
        let total = self.train_idx.len();
        o.push_str(&format!(
            "  Support vectors:  {} of {} training rows ({})\n",
            self.sv_rows.len(),
            total,
            pct(self.sv_rows.len() as f64 / total.max(1) as f64)
        ));
        let by_class: Vec<String> = self
            .d
            .classes
            .iter()
            .zip(&self.sv_by_class)
            .map(|(c, n)| format!("{c}: {n}"))
            .collect();
        o.push_str(&format!("  By class:         {}\n", by_class.join(", ")));

        if n_sub == 1 {
            let m = self.headline_model();
            o.push_str(&format!(
                "  Margin width:     {}   (2 / ||w||, in feature space)\n",
                fmt(m.margin(), d)
            ));
            o.push_str(&format!("  ||w||:            {}\n", fmt(m.w_norm, d)));
            o.push_str(&format!("  Bias (rho):       {}\n", fmt(m.rho, d)));
            o.push_str(&format!(
                "  Solver:           {} after {} iterations (tol {})\n",
                if m.converged {
                    "converged"
                } else {
                    "STOPPED at max_iter"
                },
                m.iters,
                fmt(self.opts.tol, 6)
            ));
        } else {
            let widest = self
                .d
                .classes
                .iter()
                .map(|c| c.chars().count())
                .max()
                .unwrap_or(4);
            let pw = widest * 2 + 5;
            o.push_str("\n  Sub-models\n");
            o.push_str(&format!(
                "    {}  {}  {}  {}  {}\n",
                pad("pair", pw),
                rpad("margin", 10),
                rpad("||w||", 10),
                rpad("rho", 10),
                rpad("SVs", 5)
            ));
            for m in &self.model.models {
                let name = if self.model.strategy == "ovr" {
                    format!("{} vs rest", self.d.classes[m.pos])
                } else {
                    format!("{} vs {}", self.d.classes[m.neg], self.d.classes[m.pos])
                };
                o.push_str(&format!(
                    "    {}  {}  {}  {}  {}{}\n",
                    pad(&name, pw),
                    rpad(&fmt(m.margin(), d), 10),
                    rpad(&fmt(m.w_norm, d), 10),
                    rpad(&fmt(m.rho, d), 10),
                    rpad(&(m.n_sv_pos + m.n_sv_neg).to_string(), 5),
                    if m.converged { "" } else { "  (max_iter)" }
                ));
            }
        }

        // --- linear weights
        if let Some(w) = &self.weights {
            o.push_str("\nWeights (linear kernel, on the scaled columns)\n");
            let cw = self
                .d
                .col_names
                .iter()
                .map(|c| c.chars().count())
                .max()
                .unwrap_or(4)
                .max(6);
            for (name, v) in self.d.col_names.iter().zip(w) {
                o.push_str(&format!("  {}  {}\n", pad(name, cw), rpad(&fmt(*v, d), 12)));
            }
        }

        // --- accuracy
        o.push_str("\nAccuracy\n");
        o.push_str(&format!(
            "  Training:         {} / {} = {}\n",
            self.train_metrics.correct,
            self.train_metrics.total,
            pct(self.train_metrics.accuracy())
        ));
        if let Some(t) = &self.test_metrics {
            o.push_str(&format!(
                "  Hold-out test:    {} / {} = {}\n",
                t.correct,
                t.total,
                pct(t.accuracy())
            ));
        }
        if let Some((k, acc, folds)) = &self.cv {
            o.push_str(&format!(
                "  {k}-fold CV:       {}  (folds: {})\n",
                pct(*acc),
                folds
                    .iter()
                    .map(|f| pct(*f))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }

        // --- confusion matrix
        let cm_src = self.test_metrics.as_ref().unwrap_or(&self.train_metrics);
        let cm_label = if self.test_metrics.is_some() {
            "hold-out test rows"
        } else {
            "training rows"
        };
        o.push_str(&format!(
            "\nConfusion matrix — {cm_label} (rows = actual, columns = predicted)\n"
        ));
        let lw = self
            .d
            .classes
            .iter()
            .map(|c| c.chars().count())
            .max()
            .unwrap_or(1)
            .max(6);
        let mut head = format!("  {}", pad("", lw));
        for c in &self.d.classes {
            head.push_str(&format!("  {}", rpad(c, lw)));
        }
        o.push_str(&head);
        o.push('\n');
        for (a, c) in self.d.classes.iter().enumerate() {
            let mut line = format!("  {}", pad(c, lw));
            for p in 0..self.d.classes.len() {
                line.push_str(&format!("  {}", rpad(&cm_src.cm[a][p].to_string(), lw)));
            }
            o.push_str(&line);
            o.push('\n');
        }

        // --- per-class metrics
        o.push_str("\nPer-class metrics\n");
        o.push_str(&format!(
            "  {}  {}  {}  {}  {}\n",
            pad("class", lw),
            rpad("precision", 10),
            rpad("recall", 10),
            rpad("f1", 10),
            rpad("support", 8)
        ));
        for (c, (p, r, f, s)) in self.d.classes.iter().zip(cm_src.per_class()) {
            o.push_str(&format!(
                "  {}  {}  {}  {}  {}\n",
                pad(c, lw),
                rpad(&fmt(p, d), 10),
                rpad(&fmt(r, d), 10),
                rpad(&fmt(f, d), 10),
                rpad(&s.to_string(), 8)
            ));
        }
        let (mp, mr, mf) = cm_src.macro_avg();
        o.push_str(&format!(
            "  {}  {}  {}  {}  {}\n",
            pad("macro", lw),
            rpad(&fmt(mp, d), 10),
            rpad(&fmt(mr, d), 10),
            rpad(&fmt(mf, d), 10),
            rpad(&cm_src.total.to_string(), 8)
        ));

        // --- support vectors
        o.push_str(&format!("\nSupport vectors ({})\n", self.sv_rows.len()));
        o.push_str(&format!(
            "  {}  {}  {}  {}\n",
            rpad("row", 5),
            pad("class", lw),
            rpad("alpha", 10),
            pad("bound", 8)
        ));
        let (alpha_of, bounded_of) = self.sv_alpha_map();
        for (n, &r) in self.sv_rows.iter().enumerate() {
            if n >= MAX_SV_LISTED {
                o.push_str(&format!(
                    "  … {} more support vectors not listed\n",
                    self.sv_rows.len() - MAX_SV_LISTED
                ));
                break;
            }
            o.push_str(&format!(
                "  {}  {}  {}  {}\n",
                rpad(&(r + 1).to_string(), 5),
                pad(&self.d.classes[self.d.y[r]], lw),
                rpad(&fmt(alpha_of[n], d), 10),
                pad(if bounded_of[n] { "at C" } else { "free" }, 8)
            ));
        }

        // --- predictions
        if !self.predictions.is_empty() {
            o.push_str("\nPredictions\n");
            let show_votes = self.model.strategy == "ovo" && self.d.classes.len() > 2;
            o.push_str(&format!(
                "  {}  {}  {}{}\n",
                rpad("row", 5),
                pad("class", lw),
                rpad("decision", 12),
                if show_votes { "  votes" } else { "" }
            ));
            for p in &self.predictions {
                let votes = if show_votes {
                    format!(
                        "  {}",
                        self.d
                            .classes
                            .iter()
                            .zip(&p.votes)
                            .map(|(c, v)| format!("{c}:{v}"))
                            .collect::<Vec<_>>()
                            .join(" ")
                    )
                } else {
                    String::new()
                };
                o.push_str(&format!(
                    "  {}  {}  {}{}\n",
                    rpad(&p.row.to_string(), 5),
                    pad(&self.d.classes[p.class], lw),
                    rpad(&fmt(p.decision, d), 12),
                    votes
                ));
            }
        }

        if !self.notes.is_empty() {
            o.push_str("\nNotes\n");
            for n in &self.notes {
                o.push_str(&format!("  - {n}\n"));
            }
        }

        o
    }

    /// α (and the at-bound flag) for each row in `sv_rows`, taking the largest α
    /// across sub-models so a multi-class row reports its strongest role.
    fn sv_alpha_map(&self) -> (Vec<f64>, Vec<bool>) {
        let mut alphas = Vec::with_capacity(self.sv_rows.len());
        let mut bounded = Vec::with_capacity(self.sv_rows.len());
        for &r in &self.sv_rows {
            let mut best = 0.0f64;
            let mut b = false;
            for m in &self.model.models {
                if let Some(k) = m.sv.iter().position(|&s| s == r) {
                    if m.alpha[k] > best {
                        best = m.alpha[k];
                        b = m.bounded[k];
                    }
                }
            }
            alphas.push(best);
            bounded.push(b);
        }
        (alphas, bounded)
    }

    fn json(&self) -> String {
        let d = self.opts.decimals;
        let cm_src = self.test_metrics.as_ref().unwrap_or(&self.train_metrics);
        let mut o = String::from("{\n");
        o.push_str(&format!("  \"kernel\": {},\n", json_str(self.spec.kind.label())));
        o.push_str(&format!("  \"c\": {},\n", json_num(self.opts.c, d)));
        if self.spec.kind.uses_gamma() {
            o.push_str(&format!("  \"gamma\": {},\n", json_num(self.spec.gamma, d.max(6))));
            o.push_str(&format!("  \"gamma_source\": {},\n", json_str(&self.gamma_src)));
        }
        if self.spec.kind == Kernel::Poly {
            o.push_str(&format!("  \"degree\": {},\n", self.spec.degree));
        }
        if matches!(self.spec.kind, Kernel::Poly | Kernel::Sigmoid) {
            o.push_str(&format!("  \"coef0\": {},\n", json_num(self.spec.coef0, d)));
        }
        o.push_str(&format!("  \"scaling\": {},\n", json_str(self.scaler_label)));
        o.push_str(&format!("  \"strategy\": {},\n", json_str(self.model.strategy)));
        o.push_str(&format!(
            "  \"classes\": [{}],\n",
            self.d
                .classes
                .iter()
                .map(|c| json_str(c))
                .collect::<Vec<_>>()
                .join(", ")
        ));
        o.push_str(&format!(
            "  \"feature_columns\": [{}],\n",
            self.d
                .col_names
                .iter()
                .map(|c| json_str(c))
                .collect::<Vec<_>>()
                .join(", ")
        ));
        o.push_str(&format!("  \"training_rows\": {},\n", self.train_idx.len()));
        o.push_str(&format!("  \"dropped_rows\": {},\n", self.d.dropped));
        o.push_str(&format!("  \"n_support_vectors\": {},\n", self.sv_rows.len()));
        o.push_str(&format!(
            "  \"support_vectors_by_class\": [{}],\n",
            self.sv_by_class
                .iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ));
        if self.model.models.len() == 1 {
            let m = self.headline_model();
            o.push_str(&format!("  \"margin\": {},\n", json_num(m.margin(), d)));
            o.push_str(&format!("  \"w_norm\": {},\n", json_num(m.w_norm, d)));
            o.push_str(&format!("  \"rho\": {},\n", json_num(m.rho, d)));
            o.push_str(&format!("  \"iterations\": {},\n", m.iters));
            o.push_str(&format!("  \"converged\": {},\n", m.converged));
        }
        let subs: Vec<String> = self
            .model
            .models
            .iter()
            .map(|m| {
                let name = if self.model.strategy == "ovr" {
                    format!("{} vs rest", self.d.classes[m.pos])
                } else {
                    format!("{} vs {}", self.d.classes[m.neg], self.d.classes[m.pos])
                };
                format!(
                    "    {{\"pair\": {}, \"margin\": {}, \"w_norm\": {}, \"rho\": {}, \"n_sv\": {}, \"iterations\": {}, \"converged\": {}}}",
                    json_str(&name),
                    json_num(m.margin(), d),
                    json_num(m.w_norm, d),
                    json_num(m.rho, d),
                    m.sv.len(),
                    m.iters,
                    m.converged
                )
            })
            .collect();
        o.push_str(&format!("  \"sub_models\": [\n{}\n  ],\n", subs.join(",\n")));
        if let Some(w) = &self.weights {
            o.push_str(&format!(
                "  \"weights\": {{{}}},\n",
                self.d
                    .col_names
                    .iter()
                    .zip(w)
                    .map(|(n, v)| format!("{}: {}", json_str(n), json_num(*v, d)))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        o.push_str(&format!(
            "  \"train_accuracy\": {},\n",
            json_num(self.train_metrics.accuracy(), d)
        ));
        if let Some(t) = &self.test_metrics {
            o.push_str(&format!("  \"test_accuracy\": {},\n", json_num(t.accuracy(), d)));
            o.push_str(&format!("  \"test_rows\": {},\n", t.total));
        }
        if let Some((k, acc, folds)) = &self.cv {
            o.push_str(&format!("  \"cv_folds\": {k},\n"));
            o.push_str(&format!("  \"cv_accuracy\": {},\n", json_num(*acc, d)));
            o.push_str(&format!(
                "  \"cv_fold_accuracy\": [{}],\n",
                folds
                    .iter()
                    .map(|f| json_num(*f, d))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        o.push_str(&format!(
            "  \"confusion_matrix\": [{}],\n",
            cm_src
                .cm
                .iter()
                .map(|r| format!(
                    "[{}]",
                    r.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(", ")
                ))
                .collect::<Vec<_>>()
                .join(", ")
        ));
        let pc: Vec<String> = self
            .d
            .classes
            .iter()
            .zip(cm_src.per_class())
            .map(|(c, (p, r, f, s))| {
                format!(
                    "    {{\"class\": {}, \"precision\": {}, \"recall\": {}, \"f1\": {}, \"support\": {}}}",
                    json_str(c),
                    json_num(p, d),
                    json_num(r, d),
                    json_num(f, d),
                    s
                )
            })
            .collect();
        o.push_str(&format!("  \"per_class\": [\n{}\n  ]", pc.join(",\n")));
        if !self.predictions.is_empty() {
            let ps: Vec<String> = self
                .predictions
                .iter()
                .map(|p| {
                    format!(
                        "    {{\"row\": {}, \"class\": {}, \"decision\": {}}}",
                        p.row,
                        json_str(&self.d.classes[p.class]),
                        json_num(p.decision, d)
                    )
                })
                .collect();
            o.push_str(&format!(",\n  \"predictions\": [\n{}\n  ]", ps.join(",\n")));
        }
        if !self.notes.is_empty() {
            o.push_str(&format!(
                ",\n  \"notes\": [{}]",
                self.notes
                    .iter()
                    .map(|n| json_str(n))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        o.push_str("\n}");
        o
    }

    fn csv(&self) -> String {
        let d = self.opts.decimals;
        let cm_src = self.test_metrics.as_ref().unwrap_or(&self.train_metrics);
        let mut o = String::from("section,key,value\n");
        let mut row = |s: &str, k: &str, v: String| {
            o.push_str(&format!("{},{},{}\n", csv_cell(s), csv_cell(k), csv_cell(&v)));
        };
        row("model", "kernel", self.spec.kind.label().into());
        row("model", "c", fmt(self.opts.c, d));
        if self.spec.kind.uses_gamma() {
            row("model", "gamma", fmt(self.spec.gamma, d.max(6)));
        }
        if self.spec.kind == Kernel::Poly {
            row("model", "degree", self.spec.degree.to_string());
        }
        if matches!(self.spec.kind, Kernel::Poly | Kernel::Sigmoid) {
            row("model", "coef0", fmt(self.spec.coef0, d));
        }
        row("model", "scaling", self.scaler_label.into());
        row("model", "strategy", self.model.strategy.into());
        row("model", "classes", self.d.classes.join(" "));
        row("model", "training_rows", self.train_idx.len().to_string());
        row("model", "support_vectors", self.sv_rows.len().to_string());
        if self.model.models.len() == 1 {
            let m = self.headline_model();
            row("model", "margin", fmt(m.margin(), d));
            row("model", "w_norm", fmt(m.w_norm, d));
            row("model", "rho", fmt(m.rho, d));
            row("model", "iterations", m.iters.to_string());
            row("model", "converged", m.converged.to_string());
        }
        for m in &self.model.models {
            let name = if self.model.strategy == "ovr" {
                format!("{} vs rest", self.d.classes[m.pos])
            } else {
                format!("{} vs {}", self.d.classes[m.neg], self.d.classes[m.pos])
            };
            row("sub_model", &format!("{name} margin"), fmt(m.margin(), d));
            row("sub_model", &format!("{name} n_sv"), m.sv.len().to_string());
        }
        if let Some(w) = &self.weights {
            for (n, v) in self.d.col_names.iter().zip(w) {
                row("weight", n, fmt(*v, d));
            }
        }
        row("accuracy", "train", fmt(self.train_metrics.accuracy(), d));
        if let Some(t) = &self.test_metrics {
            row("accuracy", "test", fmt(t.accuracy(), d));
        }
        if let Some((k, acc, _)) = &self.cv {
            row("accuracy", &format!("cv_{k}_fold"), fmt(*acc, d));
        }
        for (a, ca) in self.d.classes.iter().enumerate() {
            for (p, cp) in self.d.classes.iter().enumerate() {
                row(
                    "confusion",
                    &format!("actual {ca} predicted {cp}"),
                    cm_src.cm[a][p].to_string(),
                );
            }
        }
        for (c, (p, r, f, s)) in self.d.classes.iter().zip(cm_src.per_class()) {
            row("per_class", &format!("{c} precision"), fmt(p, d));
            row("per_class", &format!("{c} recall"), fmt(r, d));
            row("per_class", &format!("{c} f1"), fmt(f, d));
            row("per_class", &format!("{c} support"), s.to_string());
        }
        let (alphas, bounded) = self.sv_alpha_map();
        for (n, &r) in self.sv_rows.iter().enumerate() {
            row(
                "support_vector",
                &format!("row {}", r + 1),
                format!(
                    "{} alpha={} {}",
                    self.d.classes[self.d.y[r]],
                    fmt(alphas[n], d),
                    if bounded[n] { "at_C" } else { "free" }
                ),
            );
        }
        for p in &self.predictions {
            row(
                "prediction",
                &format!("row {}", p.row),
                format!("{} decision={}", self.d.classes[p.class], fmt(p.decision, d)),
            );
        }
        for n in &self.notes {
            row("note", "", n.clone());
        }
        o
    }
}

// -------------------------------------------------------- prediction rows ---

/// Parse the `predict` block. Three layouts are accepted: the full table layout
/// (the class column is ignored), just the feature columns in order, or a header
/// row naming the columns followed by values.
fn parse_predict(
    text: &str,
    t: &Table,
    d: &Design,
    target: usize,
    opts: &Options,
) -> Result<Vec<Vec<String>>, String> {
    let mut rows = grid(text);
    if rows.is_empty() {
        return Ok(Vec::new());
    }
    let ncol_full = t.names.len();
    let feat_cols: Vec<usize> = d.feats.iter().map(|(c, _)| *c).collect();
    let ncol_feat = feat_cols.len();

    // A header row inside `predict` remaps the column order.
    let first_is_header = rows[0]
        .iter()
        .any(|tok| t.names.iter().any(|n| n.eq_ignore_ascii_case(tok.trim())))
        && rows.len() > 1;
    let order: Option<Vec<usize>> = if first_is_header {
        let head = rows.remove(0);
        let mut map = Vec::new();
        for name in &head {
            map.push(
                t.names
                    .iter()
                    .position(|n| n.eq_ignore_ascii_case(name.trim())),
            );
        }
        for &c in &feat_cols {
            if !map.iter().any(|m| *m == Some(c)) {
                return Err(format!(
                    "rows to classify are missing the feature column '{}'",
                    t.names[c]
                ));
            }
        }
        Some(map.into_iter().map(|m| m.unwrap_or(usize::MAX)).collect())
    } else {
        None
    };

    if rows.len() > MAX_PREDICT_ROWS {
        return Err(format!(
            "too many rows to classify: {} (max {MAX_PREDICT_ROWS})",
            rows.len()
        ));
    }

    let mut out = Vec::with_capacity(rows.len());
    for (i, r) in rows.iter().enumerate() {
        // Rebuild a full-width raw row so encode_row can index by source column.
        let mut full = vec![String::new(); ncol_full];
        match &order {
            Some(map) => {
                if r.len() != map.len() {
                    return Err(format!(
                        "row {} to classify has {} values but the header names {} columns",
                        i + 1,
                        r.len(),
                        map.len()
                    ));
                }
                for (v, &c) in r.iter().zip(map) {
                    if c != usize::MAX {
                        full[c] = v.clone();
                    }
                }
            }
            None => {
                if r.len() == ncol_full {
                    full.clone_from_slice(r);
                } else if r.len() == ncol_feat {
                    for (v, &c) in r.iter().zip(&feat_cols) {
                        full[c] = v.clone();
                    }
                } else {
                    return Err(format!(
                        "row {} to classify has {} values — expected {ncol_feat} (the feature columns: {}) or {ncol_full} (the full table layout)",
                        i + 1,
                        r.len(),
                        feat_cols
                            .iter()
                            .map(|&c| t.names[c].as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ));
                }
            }
        }
        let _ = (target, opts);
        for &c in &feat_cols {
            if is_missing(&full[c]) {
                return Err(format!(
                    "row {} to classify has no value for '{}'",
                    i + 1,
                    t.names[c]
                ));
            }
        }
        out.push(full);
    }
    Ok(out)
}

// -------------------------------------------------------------------- run ---

/// Train an SVM on `data` and render the report in `opts.format`.
pub fn run(data: &str, opts: &Options) -> Result<String, String> {
    // --- validate the numeric knobs up front, with actionable messages.
    if !(opts.c.is_finite() && opts.c > 0.0) {
        return Err(format!(
            "c (cost) must be a positive number (got {}). Smaller values allow more margin violations; larger values fit the training rows harder.",
            opts.c
        ));
    }
    if !(opts.tol.is_finite() && opts.tol > 0.0) {
        return Err(format!("tol must be a positive number (got {})", opts.tol));
    }
    if opts.max_iter == 0 {
        return Err("max_iter must be at least 1".into());
    }
    if !(0.0..=0.5).contains(&opts.test_split) || !opts.test_split.is_finite() {
        return Err(format!(
            "test_split must be between 0 and 0.5 (got {})",
            opts.test_split
        ));
    }
    if opts.cv_folds == 1 || opts.cv_folds > 10 {
        return Err(format!(
            "cv_folds must be 0 (off) or between 2 and 10 (got {})",
            opts.cv_folds
        ));
    }
    if opts.decimals > 12 {
        return Err(format!("decimals must be 0–12 (got {})", opts.decimals));
    }
    let kernel = Kernel::parse(&opts.kernel)?;
    if kernel == Kernel::Poly && !(1..=10).contains(&opts.degree) {
        return Err(format!("degree must be between 1 and 10 (got {})", opts.degree));
    }
    if !opts.coef0.is_finite() {
        return Err("coef0 must be a finite number".into());
    }

    let table = parse_table(data, &opts.header)?;
    let target = resolve_column(&opts.target, &table.names, "target")?;
    let design = build_design(&table, opts)?;

    // --- scale, then cache the kernel matrix over every usable row.
    let scaler = Scaler::fit(&design.rows, &opts.scaling)?;
    let xs: Vec<Vec<f64>> = design.rows.iter().map(|r| scaler.apply(r)).collect();
    let (gamma, gamma_src) = resolve_gamma(&opts.gamma, &xs)?;
    let spec = KernelSpec {
        kind: kernel,
        gamma,
        degree: opts.degree as i32,
        coef0: opts.coef0,
    };
    let gram = Gram::build(&xs, &spec);

    let n_classes = design.classes.len();
    let all: Vec<usize> = (0..design.rows.len()).collect();
    let (train_idx, test_idx) = if opts.test_split > 0.0 {
        stratified_split(&design.y, &all, n_classes, opts.test_split, opts.seed)
    } else {
        (all.clone(), Vec::new())
    };

    let costs = class_costs(&design.y, n_classes, opts.c, &opts.class_weight)?;
    let model = train(
        &train_idx,
        &design.y,
        n_classes,
        &costs,
        &gram,
        &opts.multiclass,
        opts.tol,
        opts.max_iter,
    )?;

    let train_metrics = evaluate(&model, &train_idx, &design.y, n_classes, &gram);
    let test_metrics = if test_idx.is_empty() {
        None
    } else {
        Some(evaluate(&model, &test_idx, &design.y, n_classes, &gram))
    };

    // --- optional stratified cross-validation over the training rows.
    let cv = if opts.cv_folds >= 2 {
        let k = opts.cv_folds as usize;
        if train_idx.len() < k * 2 {
            None
        } else {
            let folds = stratified_folds(&design.y, &train_idx, n_classes, k, opts.seed);
            let mut fold_acc = Vec::with_capacity(k);
            let (mut hits, mut tot) = (0usize, 0usize);
            for f in 0..k {
                let held = &folds[f];
                if held.is_empty() {
                    continue;
                }
                let fit: Vec<usize> = train_idx
                    .iter()
                    .copied()
                    .filter(|r| !held.contains(r))
                    .collect();
                let present = (0..n_classes).filter(|c| fit.iter().any(|&r| design.y[r] == *c)).count();
                if present < 2 {
                    continue;
                }
                let m = train(
                    &fit,
                    &design.y,
                    n_classes,
                    &costs,
                    &gram,
                    &opts.multiclass,
                    opts.tol,
                    opts.max_iter,
                )?;
                let e = evaluate(&m, held, &design.y, n_classes, &gram);
                fold_acc.push(e.accuracy());
                hits += e.correct;
                tot += e.total;
            }
            if tot == 0 {
                None
            } else {
                Some((k, hits as f64 / tot as f64, fold_acc))
            }
        }
    } else {
        None
    };

    // --- support-vector union + per-class counts.
    let mut sv_rows: Vec<usize> = Vec::new();
    for m in &model.models {
        for &r in &m.sv {
            if !sv_rows.contains(&r) {
                sv_rows.push(r);
            }
        }
    }
    sv_rows.sort_unstable();
    let mut sv_by_class = vec![0usize; n_classes];
    for &r in &sv_rows {
        sv_by_class[design.y[r]] += 1;
    }

    // --- linear weights, when a single sub-model makes them unambiguous.
    let weights = if kernel == Kernel::Linear && model.models.len() == 1 {
        let m = &model.models[0];
        let mut w = vec![0.0; design.col_names.len()];
        for (k, &i) in m.sv.iter().enumerate() {
            for (j, v) in xs[i].iter().enumerate() {
                w[j] += m.coef[k] * v;
            }
        }
        Some(w)
    } else {
        None
    };

    // --- predictions for pasted rows.
    let raw_predict = parse_predict(&opts.predict, &table, &design, target, opts)?;
    let mut predictions = Vec::with_capacity(raw_predict.len());
    for (i, raw) in raw_predict.iter().enumerate() {
        let enc = encode_row(raw, &design.feats)
            .map_err(|e| format!("row {} to classify: {e}", i + 1))?;
        let scaled = scaler.apply(&enc);
        let (class, decision, votes) = model.predict_vec(&scaled, &xs, &spec);
        predictions.push(Prediction {
            row: i + 1,
            class,
            decision,
            votes,
        });
    }

    // --- notes worth surfacing above the numbers.
    let mut notes = Vec::new();
    if design.dropped > 0 {
        notes.push(format!(
            "{} row(s) dropped for a missing value in the target or a selected feature.",
            design.dropped
        ));
    }
    if !test_idx.is_empty() {
        notes.push(format!(
            "{} row(s) held out for the test split; the model was fitted on the remaining {}.",
            test_idx.len(),
            train_idx.len()
        ));
    }
    let stalled = model.models.iter().filter(|m| !m.converged).count();
    if stalled > 0 {
        notes.push(format!(
            "{stalled} sub-model(s) hit max_iter ({}) before reaching tol {} — the margin and support-vector counts are from a partially optimised fit. Raise max_iter or tol, or lower c.",
            opts.max_iter,
            fmt(opts.tol, 6)
        ));
    }
    if test_idx.is_empty() && cv.is_none() {
        notes.push(
            "Accuracy is measured on the same rows the model was fitted to, so it is optimistic. Set a test split or cross-validation folds for an honest estimate."
                .into(),
        );
    }
    if opts.scaling.trim().eq_ignore_ascii_case("none") && kernel != Kernel::Linear {
        notes.push(
            "Scaling is off. A kernel SVM compares raw distances, so a column measured in thousands will drown out one measured in single digits."
                .into(),
        );
    }
    if sv_rows.len() == train_idx.len() && train_idx.len() > 3 {
        notes.push(
            "Every training row is a support vector, which usually means gamma is too large or c is too small for this data."
                .into(),
        );
    }

    let report = Report {
        d: &design,
        opts,
        spec: &spec,
        gamma_src,
        scaler_label: scaler.label,
        model: &model,
        train_idx,
        train_metrics,
        test_metrics,
        cv,
        weights,
        sv_rows,
        sv_by_class,
        predictions,
        notes,
    };

    match opts.format.trim().to_ascii_lowercase().as_str() {
        "" | "text" => Ok(report.text()),
        "json" => Ok(report.json()),
        "csv" => Ok(report.csv()),
        other => Err(format!(
            "format must be 'text', 'json' or 'csv' (got '{other}')"
        )),
    }
}

// ------------------------------------------------------------------ tests ---

#[cfg(test)]
mod tests {
    use super::*;

    /// Two clearly separated clusters, one feature each side of a gap.
    const SEP: &str = "x,y,label\n1,1,a\n2,1,a\n1,2,a\n8,8,b\n9,8,b\n8,9,b";

    /// The report's tables are column-aligned, and the widths depend on the
    /// class/column names in play. Compare a row by its fields, not its padding.
    fn row_fields<'a>(out: &'a str, section: &str, first: &str) -> Vec<&'a str> {
        out.lines()
            .skip_while(|l| !l.starts_with(section))
            .map(|l| l.split_whitespace().collect::<Vec<_>>())
            .find(|f| f.first() == Some(&first))
            .unwrap_or_else(|| panic!("no '{first}' row under '{section}' in:\n{out}"))
    }

    fn opts(kernel: &str) -> Options {
        Options {
            kernel: kernel.into(),
            ..Default::default()
        }
    }

    #[test]
    fn linear_separable_fits_perfectly() {
        let o = opts("linear");
        let out = run(SEP, &o).unwrap();
        assert!(out.contains("Support vector machine — linear kernel"), "{out}");
        assert!(out.contains("Training:         6 / 6 = 100.00%"), "{out}");
        assert!(out.contains("Margin width:"), "{out}");
        // Both classes must contribute support vectors, or the margin is not real.
        assert!(out.contains("By class:         a: "), "{out}");
    }

    #[test]
    fn rbf_default_separates_the_same_data() {
        let out = run(SEP, &Options::default()).unwrap();
        assert!(out.contains("Support vector machine — RBF kernel"), "{out}");
        assert!(out.contains("Training:         6 / 6 = 100.00%"), "{out}");
        assert!(out.contains("gamma:            scale → "), "{out}");
    }

    /// The defining SVM invariant: on a separable problem the maximum-margin
    /// hyperplane sits exactly halfway between the closest opposing rows.
    #[test]
    fn linear_margin_matches_the_closed_form() {
        // 1-D, unscaled: classes at x = -1 and x = +1. The optimal separator is
        // x = 0 with w = 1, so the margin 2/||w|| is exactly 2.
        let data = "x,label\n-1,neg\n-1,neg\n1,pos\n1,pos";
        let o = Options {
            kernel: "linear".into(),
            scaling: "none".into(),
            c: 100.0,
            decimals: 6,
            ..Default::default()
        };
        let out = run(data, &o).unwrap();
        assert!(out.contains("Margin width:     2.000000"), "{out}");
        assert!(out.contains("||w||:            1.000000"), "{out}");
        assert!(out.contains("Bias (rho):       0.000000"), "{out}");
    }

    #[test]
    fn linear_weights_are_reported() {
        let o = Options {
            kernel: "linear".into(),
            scaling: "none".into(),
            c: 100.0,
            ..Default::default()
        };
        let out = run("x,y,label\n-1,0,neg\n-1,0,neg\n1,0,pos\n1,0,pos", &o).unwrap();
        assert!(out.contains("Weights (linear kernel"), "{out}");
        assert_eq!(row_fields(&out, "Weights", "x"), vec!["x", "1.0000"]);
        // A column with no signal must carry no weight.
        assert_eq!(row_fields(&out, "Weights", "y"), vec!["y", "0.0000"]);
    }

    #[test]
    fn predictions_follow_the_training_clusters() {
        let o = Options {
            predict: "x,y\n1,1\n9,9".into(),
            ..Default::default()
        };
        let out = run(SEP, &o).unwrap();
        let preds: Vec<&str> = out
            .lines()
            .skip_while(|l| !l.starts_with("Predictions"))
            .collect();
        let joined = preds.join("\n");
        assert_eq!(row_fields(&joined, "Predictions", "1")[1], "a");
        assert_eq!(row_fields(&joined, "Predictions", "2")[1], "b");
    }

    #[test]
    fn categorical_features_are_one_hot_encoded() {
        let data = "color,size,ripe\nred,small,yes\nred,large,yes\ngreen,small,no\ngreen,large,no";
        let out = run(data, &Options::default()).unwrap();
        assert!(out.contains("0 numeric, 4 one-hot"), "{out}");
        assert!(out.contains("Training:         4 / 4 = 100.00%"), "{out}");
    }

    #[test]
    fn multiclass_uses_one_vs_one_sub_models() {
        let data = "x,y,label\n0,0,a\n0,1,a\n5,0,b\n5,1,b\n10,0,c\n10,1,c";
        let out = run(data, &Options::default()).unwrap();
        assert!(out.contains("Multiclass:       one-vs-one (3 sub-models)"), "{out}");
        assert!(out.contains("a vs b"), "{out}");
        assert!(out.contains("b vs c"), "{out}");
    }

    #[test]
    fn multiclass_one_vs_rest_is_selectable() {
        let data = "x,y,label\n0,0,a\n0,1,a\n5,0,b\n5,1,b\n10,0,c\n10,1,c";
        let o = Options {
            multiclass: "ovr".into(),
            ..Default::default()
        };
        let out = run(data, &o).unwrap();
        assert!(out.contains("one-vs-rest (3 sub-models)"), "{out}");
        assert!(out.contains("a vs rest"), "{out}");
    }

    #[test]
    fn every_kernel_trains() {
        for k in ["linear", "rbf", "poly", "sigmoid"] {
            let out = run(SEP, &opts(k)).unwrap();
            assert!(out.contains("Support vector machine — "), "{k}: {out}");
            assert!(out.contains("Support vectors:  "), "{k}: {out}");
        }
    }

    #[test]
    fn poly_reports_degree_and_coef0() {
        let o = Options {
            kernel: "poly".into(),
            degree: 2,
            coef0: 1.0,
            ..Default::default()
        };
        let out = run(SEP, &o).unwrap();
        assert!(out.contains("degree:           2"), "{out}");
        assert!(out.contains("coef0:            1.0000"), "{out}");
    }

    #[test]
    fn gamma_accepts_scale_auto_and_a_number() {
        for (g, want) in [("scale", "scale → "), ("auto", "auto → "), ("0.25", "explicit → 0.2500")] {
            let o = Options {
                gamma: g.into(),
                ..Default::default()
            };
            let out = run(SEP, &o).unwrap();
            assert!(out.contains(want), "gamma={g}: {out}");
        }
    }

    #[test]
    fn scaling_modes_are_all_reported() {
        for (m, want) in [
            ("standard", "standard (z-score per column)"),
            ("minmax", "minmax (each column to 0–1)"),
            ("none", "none (raw values)"),
        ] {
            let o = Options {
                scaling: m.into(),
                ..Default::default()
            };
            let out = run(SEP, &o).unwrap();
            assert!(out.contains(want), "scaling={m}: {out}");
        }
    }

    #[test]
    fn balanced_class_weight_lifts_the_rare_class() {
        // 8 of one class, 2 of the other, overlapping — unweighted, the rare
        // class is easy to ignore; balanced raises its cost.
        let data = "x,label\n0,big\n0,big\n1,big\n1,big\n2,big\n2,big\n3,big\n3,big\n3,small\n4,small";
        let plain = Options {
            kernel: "linear".into(),
            c: 0.1,
            ..Default::default()
        };
        let bal = Options {
            class_weight: "balanced".into(),
            ..plain.clone()
        };
        let a = run(data, &plain).unwrap();
        let b = run(data, &bal).unwrap();
        assert!(a.contains("Accuracy"), "{a}");
        assert!(b.contains("Accuracy"), "{b}");
        // The two fits must differ — otherwise the option does nothing.
        assert_ne!(a, b);
    }

    #[test]
    fn hold_out_split_reports_a_second_accuracy() {
        let data = "x,label\n0,a\n1,a\n2,a\n3,a\n10,b\n11,b\n12,b\n13,b";
        let o = Options {
            test_split: 0.5,
            ..Default::default()
        };
        let out = run(data, &o).unwrap();
        assert!(out.contains("Hold-out test:"), "{out}");
        assert!(out.contains("row(s) held out for the test split"), "{out}");
    }

    #[test]
    fn cross_validation_reports_fold_accuracy() {
        let data = "x,label\n0,a\n1,a\n2,a\n3,a\n10,b\n11,b\n12,b\n13,b";
        let o = Options {
            cv_folds: 2,
            ..Default::default()
        };
        let out = run(data, &o).unwrap();
        assert!(out.contains("2-fold CV:"), "{out}");
        assert!(out.contains("(folds: "), "{out}");
    }

    #[test]
    fn seed_makes_the_hold_out_split_reproducible() {
        let data = "x,label\n0,a\n1,a\n2,a\n3,a\n10,b\n11,b\n12,b\n13,b";
        let mk = |seed| Options {
            test_split: 0.5,
            seed,
            ..Default::default()
        };
        assert_eq!(run(data, &mk(1)).unwrap(), run(data, &mk(1)).unwrap());
    }

    #[test]
    fn json_and_csv_formats_carry_the_headline_numbers() {
        let o = Options {
            format: "json".into(),
            ..opts("linear")
        };
        let j = run(SEP, &o).unwrap();
        assert!(j.starts_with('{') && j.ends_with('}'), "{j}");
        for key in ["\"margin\"", "\"n_support_vectors\"", "\"train_accuracy\"", "\"per_class\""] {
            assert!(j.contains(key), "missing {key} in {j}");
        }
        let o = Options {
            format: "csv".into(),
            ..opts("linear")
        };
        let c = run(SEP, &o).unwrap();
        assert!(c.starts_with("section,key,value\n"), "{c}");
        assert!(c.contains("model,margin,"), "{c}");
        assert!(c.contains("accuracy,train,"), "{c}");
    }

    #[test]
    fn decimals_control_the_printed_precision() {
        let o = Options {
            decimals: 1,
            ..opts("linear")
        };
        let out = run(SEP, &o).unwrap();
        assert!(out.contains("||w||:            0.8"), "{out}");
    }

    #[test]
    fn target_and_features_can_be_selected_by_name() {
        let data = "label,x,y,note\na,1,1,keep\na,2,1,keep\nb,8,8,drop\nb,9,8,drop";
        let o = Options {
            target: "label".into(),
            features: "x,y".into(),
            ..Default::default()
        };
        let out = run(data, &o).unwrap();
        assert!(out.contains("2 source → 2 model columns"), "{out}");
    }

    #[test]
    fn headerless_tables_get_generated_column_names() {
        let o = Options {
            header: "no".into(),
            kernel: "linear".into(),
            ..Default::default()
        };
        let out = run("1,1,0\n2,1,0\n8,8,1\n9,8,1", &o).unwrap();
        assert!(out.contains("Classes:          2 (0, 1)"), "{out}");
        assert!(out.contains("c1"), "{out}");
    }

    #[test]
    fn tab_and_semicolon_delimiters_parse() {
        for data in [
            "x\ty\tlabel\n1\t1\ta\n2\t1\ta\n8\t8\tb\n9\t8\tb",
            "x;y;label\n1;1;a\n2;1;a\n8;8;b\n9;8;b",
        ] {
            let out = run(data, &Options::default()).unwrap();
            assert!(out.contains("Classes:          2 (a, b)"), "{out}");
        }
    }

    #[test]
    fn missing_values_are_dropped_and_counted() {
        let data = "x,y,label\n1,1,a\n2,1,a\n,5,a\n8,8,b\n9,8,b\n9,?,b";
        let out = run(data, &Options::default()).unwrap();
        assert!(out.contains("Rows used:        4 of 6"), "{out}");
        assert!(out.contains("2 row(s) dropped"), "{out}");
    }

    #[test]
    fn non_convergence_is_reported_not_hidden() {
        let o = Options {
            max_iter: 1,
            c: 1000.0,
            ..Default::default()
        };
        let data = "x,y,label\n0,0,a\n1,1,b\n0,1,a\n1,0,b\n0.5,0.4,a\n0.5,0.6,b";
        let out = run(data, &o).unwrap();
        assert!(out.contains("STOPPED at max_iter") || out.contains("hit max_iter"), "{out}");
    }

    // ---- error paths

    #[test]
    fn empty_input_is_an_error() {
        let e = run("   \n  ", &Options::default()).unwrap_err();
        assert!(e.contains("no data"), "{e}");
    }

    #[test]
    fn single_class_is_an_error() {
        let e = run("x,label\n1,a\n2,a\n3,a\n4,a", &Options::default()).unwrap_err();
        assert!(e.contains("only one class"), "{e}");
    }

    #[test]
    fn one_column_is_an_error() {
        let e = run("1\n2\n3", &Options::default()).unwrap_err();
        assert!(e.contains("at least 2 columns"), "{e}");
    }

    #[test]
    fn ragged_rows_are_an_error() {
        let e = run("x,y,label\n1,1,a\n2,a", &Options::default()).unwrap_err();
        assert!(e.contains("must have the same number of columns"), "{e}");
    }

    #[test]
    fn unknown_kernel_is_an_error() {
        let e = run(SEP, &opts("quantum")).unwrap_err();
        assert!(e.contains("kernel must be"), "{e}");
    }

    #[test]
    fn non_positive_cost_is_an_error() {
        let o = Options {
            c: 0.0,
            ..Default::default()
        };
        let e = run(SEP, &o).unwrap_err();
        assert!(e.contains("c (cost) must be a positive number"), "{e}");
    }

    #[test]
    fn bad_gamma_is_an_error() {
        let o = Options {
            gamma: "wide".into(),
            ..Default::default()
        };
        let e = run(SEP, &o).unwrap_err();
        assert!(e.contains("gamma must be"), "{e}");
    }

    #[test]
    fn unknown_target_column_is_an_error() {
        let o = Options {
            target: "nope".into(),
            ..Default::default()
        };
        let e = run(SEP, &o).unwrap_err();
        assert!(e.contains("not found") && e.contains("available columns"), "{e}");
    }

    #[test]
    fn bad_format_is_an_error() {
        let o = Options {
            format: "xml".into(),
            ..Default::default()
        };
        let e = run(SEP, &o).unwrap_err();
        assert!(e.contains("format must be"), "{e}");
    }

    #[test]
    fn bad_cv_folds_is_an_error() {
        let o = Options {
            cv_folds: 1,
            ..Default::default()
        };
        let e = run(SEP, &o).unwrap_err();
        assert!(e.contains("cv_folds must be"), "{e}");
    }

    #[test]
    fn wrong_width_predict_row_is_an_error() {
        let o = Options {
            predict: "1".into(),
            ..Default::default()
        };
        let e = run(SEP, &o).unwrap_err();
        assert!(e.contains("to classify has 1 values"), "{e}");
    }

    #[test]
    fn too_many_rows_is_an_error() {
        let mut d = String::from("x,label\n");
        for i in 0..(MAX_ROWS + 2) {
            d.push_str(&format!("{i},{}\n", if i % 2 == 0 { "a" } else { "b" }));
        }
        let e = run(&d, &Options::default()).unwrap_err();
        assert!(e.contains("too many rows"), "{e}");
    }

    #[test]
    fn id_like_categorical_column_is_rejected() {
        let mut d = String::from("id,x,label\n");
        for i in 0..60 {
            d.push_str(&format!("id{i},{i},{}\n", if i % 2 == 0 { "a" } else { "b" }));
        }
        let e = run(&d, &Options::default()).unwrap_err();
        assert!(e.contains("distinct values") && e.contains("id column"), "{e}");
    }
}
