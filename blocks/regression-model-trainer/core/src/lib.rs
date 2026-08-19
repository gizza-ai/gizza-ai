//! regression-model-trainer core — pure compute, shared by the chat skill block and the web page.
//! No wafer/wasm-bindgen deps.
//!
//! Trains a regression model that predicts one numeric column of a pasted table
//! from the other numeric columns, and reports R², RMSE, MAE, the coefficients
//! (linear/ridge) or the feature importances (random forest), with an optional
//! held-out test split and k-fold cross-validation. Everything is deterministic:
//! the bootstrap sampling, the shuffling for the split and the CV folds are all
//! driven by the `seed` option.

/// Hard caps — keep a pasted table inside what a browser tab can chew through.
pub const MAX_ROWS: usize = 20_000;
pub const MAX_COLS: usize = 100;
/// Random forest work budget: rows × trees × (1 + cv_folds) must stay under this.
pub const MAX_FOREST_WORK: usize = 1_000_000;

/// Every knob the tool exposes. `Default` mirrors the descriptor defaults.
#[derive(Clone, Debug)]
pub struct Options {
    pub target: String,
    pub features: String,
    pub model: String,
    pub alpha: f64,
    pub standardize: bool,
    pub trees: u32,
    pub max_depth: u32,
    pub test_split: f64,
    pub cv_folds: u32,
    pub seed: u64,
    pub header: String,
    pub decimals: u32,
    pub format: String,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            target: "last".into(),
            features: String::new(),
            model: "linear".into(),
            alpha: 1.0,
            standardize: true,
            trees: 100,
            max_depth: 8,
            test_split: 0.0,
            cv_folds: 0,
            seed: 42,
            header: "auto".into(),
            decimals: 4,
            format: "text".into(),
        }
    }
}

/// Deterministic xorshift64* — the only randomness in the tool, so a `seed`
/// reproduces a forest, a split and a fold assignment exactly.
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
    fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }
    fn shuffle(&mut self, v: &mut [usize]) {
        for i in (1..v.len()).rev() {
            v.swap(i, self.below(i + 1));
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
    tok.trim().parse::<f64>().is_ok()
}

/// Pick the column delimiter from the first non-blank line: whichever of
/// comma / tab / semicolon / pipe occurs most, else any run of whitespace.
fn split_row(line: &str, delim: Option<char>) -> Vec<String> {
    match delim {
        Some(d) => line.split(d).map(|s| s.trim().to_string()).collect(),
        None => line.split_whitespace().map(|s| s.to_string()).collect(),
    }
}

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

struct Table {
    names: Vec<String>,
    rows: Vec<Vec<String>>,
}

fn parse_table(data: &str, header_mode: &str) -> Result<Table, String> {
    let lines: Vec<&str> = data
        .lines()
        .map(|l| l.trim_end_matches('\r'))
        .filter(|l| !l.trim().is_empty())
        .collect();
    if lines.is_empty() {
        return Err(
            "no data: paste a table with one row per observation, e.g. 'x,y' then '1,3'".into(),
        );
    }
    let delim = detect_delim(lines[0]);
    let mut rows: Vec<Vec<String>> = lines.iter().map(|l| split_row(l, delim)).collect();
    let ncol = rows[0].len();
    if ncol < 2 {
        return Err(format!(
            "need at least 2 columns (predictors + the target), found {ncol}. Separate columns with commas, tabs, semicolons or spaces."
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
        return Err(format!("too many rows: {} (max {MAX_ROWS})", rows.len()));
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

// ------------------------------------------------------------- lin algebra ---

/// Solve A·x = b in place by Gaussian elimination with partial pivoting.
fn solve(mut a: Vec<Vec<f64>>, mut b: Vec<f64>) -> Option<Vec<f64>> {
    let n = b.len();
    for k in 0..n {
        let mut piv = k;
        for i in k + 1..n {
            if a[i][k].abs() > a[piv][k].abs() {
                piv = i;
            }
        }
        if a[piv][k].abs() < 1e-11 {
            return None;
        }
        a.swap(k, piv);
        b.swap(k, piv);
        for i in k + 1..n {
            let f = a[i][k] / a[k][k];
            if f == 0.0 {
                continue;
            }
            for j in k..n {
                a[i][j] -= f * a[k][j];
            }
            b[i] -= f * b[k];
        }
    }
    let mut x = vec![0.0; n];
    for i in (0..n).rev() {
        let mut s = b[i];
        for j in i + 1..n {
            s -= a[i][j] * x[j];
        }
        x[i] = s / a[i][i];
    }
    if x.iter().all(|v| v.is_finite()) {
        Some(x)
    } else {
        None
    }
}

/// Fitted linear model in ORIGINAL units: `intercept + Σ coef_j · x_j`.
#[derive(Clone, Debug)]
struct LinearModel {
    intercept: f64,
    coef: Vec<f64>,
}

impl LinearModel {
    fn predict(&self, x: &[Vec<f64>], row: usize) -> f64 {
        let mut s = self.intercept;
        for (j, c) in self.coef.iter().enumerate() {
            s += c * x[j][row];
        }
        s
    }
}

/// Ridge (α > 0) / OLS (α = 0) with an unpenalised intercept. Predictors are
/// always centred; `standardize` additionally scales them to unit variance so
/// the L2 penalty hits differently-scaled columns evenly. Coefficients are
/// back-transformed to the original units either way.
fn fit_linear(
    x: &[Vec<f64>],
    y: &[f64],
    rows: &[usize],
    alpha: f64,
    standardize: bool,
) -> Result<LinearModel, String> {
    let p = x.len();
    let n = rows.len();
    if n == 0 {
        return Err("no rows to fit".into());
    }
    let mut mean = vec![0.0; p];
    let mut sd = vec![1.0; p];
    for j in 0..p {
        let m = rows.iter().map(|&r| x[j][r]).sum::<f64>() / n as f64;
        mean[j] = m;
        if standardize {
            let var = rows.iter().map(|&r| (x[j][r] - m).powi(2)).sum::<f64>() / n as f64;
            sd[j] = if var > 0.0 { var.sqrt() } else { 1.0 };
        }
    }
    let ybar = rows.iter().map(|&r| y[r]).sum::<f64>() / n as f64;

    // Normal equations on the centred/scaled design.
    let mut xtx = vec![vec![0.0; p]; p];
    let mut xty = vec![0.0; p];
    for &r in rows {
        let z: Vec<f64> = (0..p).map(|j| (x[j][r] - mean[j]) / sd[j]).collect();
        let yc = y[r] - ybar;
        for j in 0..p {
            xty[j] += z[j] * yc;
            for k in j..p {
                xtx[j][k] += z[j] * z[k];
            }
        }
    }
    for j in 0..p {
        for k in 0..j {
            xtx[j][k] = xtx[k][j];
        }
        xtx[j][j] += alpha;
    }

    let b = solve(xtx, xty).ok_or_else(|| {
        if alpha > 0.0 {
            "the penalised system could not be solved — check for duplicate or constant columns"
                .to_string()
        } else {
            "predictors are perfectly collinear (one column is an exact linear combination of the others) — drop the redundant column or switch model to 'ridge' with alpha > 0".to_string()
        }
    })?;

    let coef: Vec<f64> = (0..p).map(|j| b[j] / sd[j]).collect();
    let intercept = ybar - (0..p).map(|j| coef[j] * mean[j]).sum::<f64>();
    Ok(LinearModel { intercept, coef })
}

// ------------------------------------------------------------ random forest ---

enum Node {
    Leaf(f64),
    Split {
        feature: usize,
        threshold: f64,
        left: usize,
        right: usize,
    },
}

struct Tree {
    nodes: Vec<Node>,
}

impl Tree {
    fn predict(&self, x: &[Vec<f64>], row: usize) -> f64 {
        let mut i = 0usize;
        loop {
            match &self.nodes[i] {
                Node::Leaf(v) => return *v,
                Node::Split {
                    feature,
                    threshold,
                    left,
                    right,
                } => {
                    i = if x[*feature][row] <= *threshold {
                        *left
                    } else {
                        *right
                    };
                }
            }
        }
    }
}

struct SplitCandidate {
    feature: usize,
    threshold: f64,
    gain: f64,
    left: Vec<usize>,
    right: Vec<usize>,
}

/// Exhaustive CART split search over `mtry` randomly chosen features: sort the
/// node's rows by the feature and scan every value boundary, keeping the split
/// with the biggest sum-of-squared-error reduction.
fn best_split(
    x: &[Vec<f64>],
    y: &[f64],
    rows: &[usize],
    mtry: usize,
    rng: &mut Rng,
) -> Option<SplitCandidate> {
    let p = x.len();
    let n = rows.len();
    let total: f64 = rows.iter().map(|&r| y[r]).sum();
    let total_sq: f64 = rows.iter().map(|&r| y[r] * y[r]).sum();
    let parent_sse = total_sq - total * total / n as f64;

    // Sample `mtry` distinct features without replacement (partial shuffle).
    let mut feats: Vec<usize> = (0..p).collect();
    for i in 0..mtry.min(p) {
        let j = i + rng.below(p - i);
        feats.swap(i, j);
    }

    let mut best: Option<SplitCandidate> = None;
    let mut buf: Vec<(f64, f64)> = Vec::with_capacity(n);
    for &f in feats.iter().take(mtry.min(p)) {
        buf.clear();
        buf.extend(rows.iter().map(|&r| (x[f][r], y[r])));
        buf.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        if buf[0].0 == buf[n - 1].0 {
            continue; // constant within this node
        }
        let (mut ls, mut lsq, mut lc) = (0.0f64, 0.0f64, 0usize);
        for i in 0..n - 1 {
            ls += buf[i].1;
            lsq += buf[i].1 * buf[i].1;
            lc += 1;
            if buf[i].0 == buf[i + 1].0 {
                continue;
            }
            let rc = n - lc;
            let rs = total - ls;
            let rsq = total_sq - lsq;
            let sse = (lsq - ls * ls / lc as f64) + (rsq - rs * rs / rc as f64);
            let gain = parent_sse - sse;
            if gain > best.as_ref().map(|b| b.gain).unwrap_or(1e-12) {
                let threshold = (buf[i].0 + buf[i + 1].0) / 2.0;
                let (mut l, mut r) = (Vec::with_capacity(lc), Vec::with_capacity(rc));
                for &row in rows {
                    if x[f][row] <= threshold {
                        l.push(row)
                    } else {
                        r.push(row)
                    }
                }
                if l.is_empty() || r.is_empty() {
                    continue;
                }
                best = Some(SplitCandidate {
                    feature: f,
                    threshold,
                    gain,
                    left: l,
                    right: r,
                });
            }
        }
    }
    best
}

fn build_tree(
    x: &[Vec<f64>],
    y: &[f64],
    rows: Vec<usize>,
    depth: u32,
    max_depth: u32,
    mtry: usize,
    rng: &mut Rng,
    nodes: &mut Vec<Node>,
    importance: &mut [f64],
) -> usize {
    let idx = nodes.len();
    let mean = rows.iter().map(|&r| y[r]).sum::<f64>() / rows.len() as f64;
    nodes.push(Node::Leaf(mean));
    if depth >= max_depth || rows.len() < 2 {
        return idx;
    }
    let Some(sp) = best_split(x, y, &rows, mtry, rng) else {
        return idx;
    };
    importance[sp.feature] += sp.gain;
    let left = build_tree(
        x,
        y,
        sp.left,
        depth + 1,
        max_depth,
        mtry,
        rng,
        nodes,
        importance,
    );
    let right = build_tree(
        x,
        y,
        sp.right,
        depth + 1,
        max_depth,
        mtry,
        rng,
        nodes,
        importance,
    );
    nodes[idx] = Node::Split {
        feature: sp.feature,
        threshold: sp.threshold,
        left,
        right,
    };
    idx
}

struct Forest {
    trees: Vec<Tree>,
    importance: Vec<f64>,
    /// Out-of-bag prediction per training row (NaN when a row was in every bag).
    oob: Vec<f64>,
}

impl Forest {
    fn predict(&self, x: &[Vec<f64>], row: usize) -> f64 {
        self.trees.iter().map(|t| t.predict(x, row)).sum::<f64>() / self.trees.len() as f64
    }
}

fn fit_forest(
    x: &[Vec<f64>],
    y: &[f64],
    rows: &[usize],
    trees: u32,
    max_depth: u32,
    seed: u64,
) -> Forest {
    let p = x.len();
    let mtry = ((p as f64 / 3.0).round() as usize).clamp(1, p);
    let n = rows.len();
    let mut importance = vec![0.0; p];
    let mut forest = Vec::with_capacity(trees as usize);
    // OOB accumulators are indexed by position within `rows`.
    let mut oob_sum = vec![0.0; n];
    let mut oob_cnt = vec![0usize; n];

    for t in 0..trees {
        let mut rng = Rng::new(seed ^ ((t as u64 + 1).wrapping_mul(0x9e37_79b9_7f4a_7c15)));
        let mut in_bag = vec![false; n];
        let mut bag = Vec::with_capacity(n);
        for _ in 0..n {
            let i = rng.below(n);
            in_bag[i] = true;
            bag.push(rows[i]);
        }
        let mut nodes = Vec::new();
        build_tree(
            x,
            y,
            bag,
            0,
            max_depth,
            mtry,
            &mut rng,
            &mut nodes,
            &mut importance,
        );
        let tree = Tree { nodes };
        for (i, &r) in rows.iter().enumerate() {
            if !in_bag[i] {
                oob_sum[i] += tree.predict(x, r);
                oob_cnt[i] += 1;
            }
        }
        forest.push(tree);
    }

    let oob = (0..n)
        .map(|i| {
            if oob_cnt[i] > 0 {
                oob_sum[i] / oob_cnt[i] as f64
            } else {
                f64::NAN
            }
        })
        .collect();
    Forest {
        trees: forest,
        importance,
        oob,
    }
}

// ----------------------------------------------------------------- metrics ---

#[derive(Clone, Copy, Debug, Default)]
struct Metrics {
    n: usize,
    r2: f64,
    rmse: f64,
    mae: f64,
}

fn metrics(actual: &[f64], pred: &[f64]) -> Metrics {
    let n = actual.len();
    if n == 0 {
        return Metrics::default();
    }
    let mean = actual.iter().sum::<f64>() / n as f64;
    let mut sse = 0.0;
    let mut sst = 0.0;
    let mut abs = 0.0;
    for i in 0..n {
        let e = actual[i] - pred[i];
        sse += e * e;
        abs += e.abs();
        sst += (actual[i] - mean).powi(2);
    }
    Metrics {
        n,
        r2: if sst > 0.0 { 1.0 - sse / sst } else { f64::NAN },
        rmse: (sse / n as f64).sqrt(),
        mae: abs / n as f64,
    }
}

// -------------------------------------------------------------- formatting ---

fn fmt_num(v: f64, d: u32) -> String {
    if !v.is_finite() {
        return "n/a".into();
    }
    let s = format!("{:.*}", d as usize, v);
    let s = if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s
    };
    if s == "-0" {
        "0".into()
    } else {
        s
    }
}

fn json_str(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    o.push('"');
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

fn json_num(v: f64) -> String {
    if v.is_finite() {
        format!("{v}")
    } else {
        "null".into()
    }
}

fn csv_cell(s: &str) -> String {
    if s.contains([',', '"', '\n']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

// -------------------------------------------------------------------- run ---

struct Fit {
    model: String,
    train: Metrics,
    test: Option<Metrics>,
    oob: Option<Metrics>,
    cv: Option<Metrics>,
    coef: Option<(f64, Vec<f64>)>,
    importance: Option<Vec<f64>>,
}

/// Fit `model` on `train_rows` and return a predictor closure applied to any row.
fn fit_predictor<'a>(
    x: &'a [Vec<f64>],
    y: &'a [f64],
    train_rows: &[usize],
    o: &Options,
    model: &str,
) -> Result<Box<dyn Fn(usize) -> f64 + 'a>, String> {
    match model {
        "random_forest" => {
            let f = fit_forest(x, y, train_rows, o.trees, o.max_depth, o.seed);
            Ok(Box::new(move |r| f.predict(x, r)))
        }
        _ => {
            let alpha = if model == "ridge" { o.alpha } else { 0.0 };
            let m = fit_linear(x, y, train_rows, alpha, o.standardize)?;
            Ok(Box::new(move |r| m.predict(x, r)))
        }
    }
}

/// Fit the requested model, predict a numeric column and report the fit.
pub fn run(data: &str, o: &Options) -> Result<String, String> {
    let model = o.model.trim().to_ascii_lowercase().replace(['-', ' '], "_");
    let model = match model.as_str() {
        "" | "linear" | "ols" => "linear",
        "ridge" | "l2" => "ridge",
        "random_forest" | "forest" | "rf" => "random_forest",
        other => {
            return Err(format!(
                "model must be 'linear', 'ridge' or 'random_forest' (got '{other}')"
            ))
        }
    };
    let format = match o.format.trim().to_ascii_lowercase().as_str() {
        "" | "text" => "text",
        "json" => "json",
        "csv" => "csv",
        other => {
            return Err(format!(
                "format must be 'text', 'json' or 'csv' (got '{other}')"
            ))
        }
    };
    if !(0.0..=1e9).contains(&o.alpha) {
        return Err(format!(
            "alpha must be between 0 and 1000000000 (got {})",
            o.alpha
        ));
    }
    if !(0.0..=0.5).contains(&o.test_split) {
        return Err(format!(
            "test_split must be between 0 and 0.5 (got {})",
            o.test_split
        ));
    }
    if o.cv_folds == 1 || o.cv_folds > 10 {
        return Err(format!(
            "cv_folds must be 0 (off) or between 2 and 10 (got {})",
            o.cv_folds
        ));
    }
    if o.trees < 1 || o.trees > 300 {
        return Err(format!("trees must be between 1 and 300 (got {})", o.trees));
    }
    if o.max_depth < 1 || o.max_depth > 20 {
        return Err(format!(
            "max_depth must be between 1 and 20 (got {})",
            o.max_depth
        ));
    }
    if o.decimals > 12 {
        return Err(format!(
            "decimals must be between 0 and 12 (got {})",
            o.decimals
        ));
    }

    let table = parse_table(data, &o.header)?;
    let names = &table.names;
    let ti = resolve_column(&o.target, names, "target")?;
    let feature_idx: Vec<usize> = if o.features.trim().is_empty() {
        (0..names.len()).filter(|&i| i != ti).collect()
    } else {
        let mut v = Vec::new();
        for tok in o.features.split(',') {
            if tok.trim().is_empty() {
                continue;
            }
            let i = resolve_column(tok, names, "feature")?;
            if i == ti {
                return Err(format!(
                    "'{}' is both the target and a feature — remove it from the feature list",
                    names[i]
                ));
            }
            if !v.contains(&i) {
                v.push(i);
            }
        }
        v
    };
    if feature_idx.is_empty() {
        return Err("no feature columns selected — the table needs at least one predictor besides the target".into());
    }

    // Numeric matrix; rows with a missing value in a used column are dropped.
    let mut x: Vec<Vec<f64>> = vec![Vec::new(); feature_idx.len()];
    let mut y: Vec<f64> = Vec::new();
    let mut dropped = 0usize;
    'rows: for (ri, row) in table.rows.iter().enumerate() {
        let mut vals = Vec::with_capacity(feature_idx.len() + 1);
        for &ci in feature_idx.iter().chain(std::iter::once(&ti)) {
            let tok = &row[ci];
            if is_missing(tok) {
                dropped += 1;
                continue 'rows;
            }
            match tok.trim().replace(['%', '$', ','], "").parse::<f64>() {
                Ok(v) if v.is_finite() => vals.push(v),
                _ => {
                    return Err(format!(
                        "row {}, column '{}': '{}' is not a number — every predictor and the target must be numeric (encode categories as 0/1 columns)",
                        ri + 1,
                        names[ci],
                        tok
                    ))
                }
            }
        }
        for (j, v) in vals.iter().take(feature_idx.len()).enumerate() {
            x[j].push(*v);
        }
        y.push(vals[feature_idx.len()]);
    }
    let n = y.len();
    let p = feature_idx.len();
    if n < 3 {
        return Err(format!(
            "need at least 3 usable rows, found {n}{}",
            if dropped > 0 {
                format!(" ({dropped} row(s) dropped for missing values)")
            } else {
                String::new()
            }
        ));
    }

    // Deterministic hold-out split.
    let mut order: Vec<usize> = (0..n).collect();
    let n_test = (n as f64 * o.test_split).floor() as usize;
    let (train_rows, test_rows): (Vec<usize>, Vec<usize>) = if n_test >= 1 {
        let mut rng = Rng::new(o.seed ^ 0x5f3a_1c7b);
        rng.shuffle(&mut order);
        let test: Vec<usize> = order[..n_test].to_vec();
        let mut train: Vec<usize> = order[n_test..].to_vec();
        train.sort_unstable();
        let mut test = test;
        test.sort_unstable();
        (train, test)
    } else {
        (order.clone(), Vec::new())
    };

    if model != "random_forest" {
        let need = p + 1;
        if train_rows.len() < need && model == "linear" {
            return Err(format!(
                "linear regression needs more training rows than predictors: {} row(s) for {p} predictor(s). Add rows, drop predictors, or use model='ridge'.",
                train_rows.len()
            ));
        }
        for (j, col) in x.iter().enumerate() {
            let first = col[train_rows[0]];
            if train_rows.iter().all(|&r| col[r] == first) {
                return Err(format!(
                    "predictor '{}' is constant across the training rows — drop it",
                    names[feature_idx[j]]
                ));
            }
        }
    } else {
        let work = train_rows.len() * o.trees as usize * (1 + o.cv_folds as usize);
        if work > MAX_FOREST_WORK {
            return Err(format!(
                "random forest work budget exceeded: {} training rows × {} trees × {} fit(s) = {work} (max {MAX_FOREST_WORK}). Lower trees, cv_folds, or use fewer rows.",
                train_rows.len(),
                o.trees,
                1 + o.cv_folds
            ));
        }
    }

    // ---- fit on the training rows -------------------------------------------
    let mut coef = None;
    let mut importance = None;
    let mut oob_metrics = None;
    let train_pred: Vec<f64>;
    let test_pred: Vec<f64>;
    if model == "random_forest" {
        let f = fit_forest(&x, &y, &train_rows, o.trees, o.max_depth, o.seed);
        train_pred = train_rows.iter().map(|&r| f.predict(&x, r)).collect();
        test_pred = test_rows.iter().map(|&r| f.predict(&x, r)).collect();
        let total: f64 = f.importance.iter().sum();
        importance = Some(if total > 0.0 {
            f.importance.iter().map(|v| v / total).collect()
        } else {
            vec![0.0; p]
        });
        let (oa, op): (Vec<f64>, Vec<f64>) = train_rows
            .iter()
            .zip(f.oob.iter())
            .filter(|(_, &pr)| pr.is_finite())
            .map(|(&r, &pr)| (y[r], pr))
            .unzip();
        if !oa.is_empty() {
            oob_metrics = Some(metrics(&oa, &op));
        }
    } else {
        let alpha = if model == "ridge" { o.alpha } else { 0.0 };
        let m = fit_linear(&x, &y, &train_rows, alpha, o.standardize)?;
        train_pred = train_rows.iter().map(|&r| m.predict(&x, r)).collect();
        test_pred = test_rows.iter().map(|&r| m.predict(&x, r)).collect();
        coef = Some((m.intercept, m.coef.clone()));
    }
    let train_actual: Vec<f64> = train_rows.iter().map(|&r| y[r]).collect();
    let train = metrics(&train_actual, &train_pred);
    let test = if test_rows.is_empty() {
        None
    } else {
        let a: Vec<f64> = test_rows.iter().map(|&r| y[r]).collect();
        Some(metrics(&a, &test_pred))
    };

    // ---- k-fold cross-validation on the training rows ------------------------
    let cv = if o.cv_folds >= 2 {
        let k = o.cv_folds as usize;
        if train_rows.len() < k {
            return Err(format!(
                "cv_folds={k} needs at least {k} training rows, found {}",
                train_rows.len()
            ));
        }
        let mut shuffled = train_rows.clone();
        Rng::new(o.seed ^ 0x2b1c_9d4e).shuffle(&mut shuffled);
        let mut actual = Vec::with_capacity(shuffled.len());
        let mut pred = Vec::with_capacity(shuffled.len());
        for f in 0..k {
            let hold: Vec<usize> = shuffled
                .iter()
                .enumerate()
                .filter(|(i, _)| i % k == f)
                .map(|(_, &r)| r)
                .collect();
            let fit_rows: Vec<usize> = shuffled
                .iter()
                .enumerate()
                .filter(|(i, _)| i % k != f)
                .map(|(_, &r)| r)
                .collect();
            if hold.is_empty() || fit_rows.is_empty() {
                continue;
            }
            if model == "linear" && fit_rows.len() < p + 1 {
                return Err(format!(
                    "cv_folds={k} leaves only {} rows per fit for {p} predictor(s) — use fewer folds or model='ridge'",
                    fit_rows.len()
                ));
            }
            let predict = fit_predictor(&x, &y, &fit_rows, o, model)?;
            for &r in &hold {
                actual.push(y[r]);
                pred.push(predict(r));
            }
        }
        Some(metrics(&actual, &pred))
    } else {
        None
    };

    let fit = Fit {
        model: model.to_string(),
        train,
        test,
        oob: oob_metrics,
        cv,
        coef,
        importance,
    };

    let fnames: Vec<String> = feature_idx.iter().map(|&i| names[i].clone()).collect();
    Ok(match format {
        "json" => render_json(&fit, &names[ti], &fnames, n, dropped, o),
        "csv" => render_csv(&fit, &names[ti], &fnames, n, dropped, o),
        _ => render_text(&fit, &names[ti], &fnames, n, dropped, o),
    })
}

fn model_label(fit: &Fit, o: &Options) -> String {
    match fit.model.as_str() {
        "ridge" => format!(
            "Ridge regression (L2, alpha = {}{})",
            fmt_num(o.alpha, o.decimals.max(2)),
            if o.standardize {
                ", standardized predictors"
            } else {
                ", unstandardized predictors"
            }
        ),
        "random_forest" => format!(
            "Random forest regression ({} trees, max depth {})",
            o.trees, o.max_depth
        ),
        _ => "Linear regression (OLS)".to_string(),
    }
}

fn metric_block(label: &str, m: &Metrics, d: u32) -> String {
    format!(
        "{label} (n = {}):\n  R²    {}\n  RMSE  {}\n  MAE   {}\n",
        m.n,
        fmt_num(m.r2, d),
        fmt_num(m.rmse, d),
        fmt_num(m.mae, d)
    )
}

fn render_text(
    fit: &Fit,
    target: &str,
    features: &[String],
    n: usize,
    dropped: usize,
    o: &Options,
) -> String {
    let d = o.decimals;
    let mut s = String::new();
    s.push_str(&format!("{}\n", model_label(fit, o)));
    s.push_str(&format!("Target: {target}\n"));
    s.push_str(&format!(
        "Predictors ({}): {}\n",
        features.len(),
        features.join(", ")
    ));
    s.push_str(&format!(
        "Rows: {n} used{}{}\n\n",
        if dropped > 0 {
            format!(" ({dropped} dropped for missing values)")
        } else {
            String::new()
        },
        match &fit.test {
            Some(t) => format!(" — train {}, test {}", fit.train.n, t.n),
            None => String::new(),
        }
    ));

    s.push_str(&metric_block("Training fit", &fit.train, d));
    if let Some(t) = &fit.test {
        s.push('\n');
        s.push_str(&metric_block("Held-out test", t, d));
    }
    if let Some(ob) = &fit.oob {
        s.push('\n');
        s.push_str(&metric_block("Out-of-bag", ob, d));
    }
    if let Some(cv) = &fit.cv {
        s.push('\n');
        s.push_str(&metric_block(
            &format!("{}-fold cross-validation", o.cv_folds),
            cv,
            d,
        ));
    }

    if let Some((intercept, coefs)) = &fit.coef {
        let width = features
            .iter()
            .map(|f| f.len())
            .chain(std::iter::once("(Intercept)".len()))
            .max()
            .unwrap_or(11);
        s.push_str("\nCoefficients:\n");
        s.push_str(&format!(
            "  {:<width$}  {}\n",
            "(Intercept)",
            fmt_num(*intercept, d),
            width = width
        ));
        for (f, c) in features.iter().zip(coefs) {
            s.push_str(&format!(
                "  {:<width$}  {}\n",
                f,
                fmt_num(*c, d),
                width = width
            ));
        }
        let mut eq = format!("\nEquation: {target} = {}", fmt_num(*intercept, d));
        for (f, c) in features.iter().zip(coefs) {
            eq.push_str(&format!(
                " {} {}·{f}",
                if *c < 0.0 { "-" } else { "+" },
                fmt_num(c.abs(), d)
            ));
        }
        s.push_str(&eq);
        s.push('\n');
    }
    if let Some(imp) = &fit.importance {
        let width = features.iter().map(|f| f.len()).max().unwrap_or(8);
        let mut ranked: Vec<(usize, f64)> = imp.iter().cloned().enumerate().collect();
        ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        s.push_str("\nFeature importance (share of total variance reduction):\n");
        for (i, v) in ranked {
            s.push_str(&format!(
                "  {:<width$}  {}%\n",
                features[i],
                fmt_num(v * 100.0, d.min(2)),
                width = width
            ));
        }
    }
    s.trim_end().to_string()
}

fn render_json(
    fit: &Fit,
    target: &str,
    features: &[String],
    n: usize,
    dropped: usize,
    o: &Options,
) -> String {
    let met = |m: &Metrics| {
        format!(
            "{{\"n\":{},\"r2\":{},\"rmse\":{},\"mae\":{}}}",
            m.n,
            json_num(m.r2),
            json_num(m.rmse),
            json_num(m.mae)
        )
    };
    let mut s = String::from("{");
    s.push_str(&format!("\"model\":{},", json_str(&fit.model)));
    s.push_str(&format!("\"target\":{},", json_str(target)));
    s.push_str(&format!(
        "\"features\":[{}],",
        features
            .iter()
            .map(|f| json_str(f))
            .collect::<Vec<_>>()
            .join(",")
    ));
    s.push_str(&format!("\"rows_used\":{n},\"rows_dropped\":{dropped},"));
    if fit.model == "ridge" {
        s.push_str(&format!(
            "\"alpha\":{},\"standardize\":{},",
            json_num(o.alpha),
            o.standardize
        ));
    }
    if fit.model == "random_forest" {
        s.push_str(&format!(
            "\"trees\":{},\"max_depth\":{},\"seed\":{},",
            o.trees, o.max_depth, o.seed
        ));
    }
    s.push_str(&format!("\"train\":{}", met(&fit.train)));
    if let Some(t) = &fit.test {
        s.push_str(&format!(",\"test\":{}", met(t)));
    }
    if let Some(ob) = &fit.oob {
        s.push_str(&format!(",\"oob\":{}", met(ob)));
    }
    if let Some(cv) = &fit.cv {
        s.push_str(&format!(",\"cv\":{}", met(cv)));
    }
    if let Some((intercept, coefs)) = &fit.coef {
        s.push_str(&format!(",\"intercept\":{}", json_num(*intercept)));
        s.push_str(",\"coefficients\":{");
        s.push_str(
            &features
                .iter()
                .zip(coefs)
                .map(|(f, c)| format!("{}:{}", json_str(f), json_num(*c)))
                .collect::<Vec<_>>()
                .join(","),
        );
        s.push('}');
    }
    if let Some(imp) = &fit.importance {
        s.push_str(",\"importance\":{");
        s.push_str(
            &features
                .iter()
                .zip(imp)
                .map(|(f, v)| format!("{}:{}", json_str(f), json_num(*v)))
                .collect::<Vec<_>>()
                .join(","),
        );
        s.push('}');
    }
    s.push('}');
    s
}

fn render_csv(
    fit: &Fit,
    target: &str,
    features: &[String],
    n: usize,
    dropped: usize,
    o: &Options,
) -> String {
    let d = o.decimals;
    let mut s = String::from("section,name,value\n");
    s.push_str(&format!("model,type,{}\n", fit.model));
    s.push_str(&format!("model,target,{}\n", csv_cell(target)));
    s.push_str(&format!("model,rows_used,{n}\n"));
    s.push_str(&format!("model,rows_dropped,{dropped}\n"));
    let push = |sec: &str, m: &Metrics, s: &mut String| {
        s.push_str(&format!("{sec},n,{}\n", m.n));
        s.push_str(&format!("{sec},r2,{}\n", fmt_num(m.r2, d)));
        s.push_str(&format!("{sec},rmse,{}\n", fmt_num(m.rmse, d)));
        s.push_str(&format!("{sec},mae,{}\n", fmt_num(m.mae, d)));
    };
    push("train", &fit.train, &mut s);
    if let Some(t) = &fit.test {
        push("test", t, &mut s);
    }
    if let Some(ob) = &fit.oob {
        push("oob", ob, &mut s);
    }
    if let Some(cv) = &fit.cv {
        push("cv", cv, &mut s);
    }
    if let Some((intercept, coefs)) = &fit.coef {
        s.push_str(&format!(
            "coefficient,(Intercept),{}\n",
            fmt_num(*intercept, d)
        ));
        for (f, c) in features.iter().zip(coefs) {
            s.push_str(&format!("coefficient,{},{}\n", csv_cell(f), fmt_num(*c, d)));
        }
    }
    if let Some(imp) = &fit.importance {
        for (f, v) in features.iter().zip(imp) {
            s.push_str(&format!("importance,{},{}\n", csv_cell(f), fmt_num(*v, d)));
        }
    }
    s.trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(model: &str) -> Options {
        Options {
            model: model.into(),
            ..Default::default()
        }
    }

    #[test]
    fn linear_recovers_exact_line() {
        let out = run("x,y\n1,3\n2,5\n3,7\n4,9", &opts("linear")).unwrap();
        assert!(out.contains("Linear regression (OLS)"), "{out}");
        assert!(out.contains("Equation: y = 1 + 2·x"), "{out}");
        assert!(out.contains("R²    1"), "{out}");
        assert!(out.contains("RMSE  0"), "{out}");
    }

    #[test]
    fn linear_multi_predictor_matches_ols() {
        let o = Options {
            decimals: 6,
            ..opts("linear")
        };
        let out = run("1,1,6\n2,1,8\n3,2,11\n4,2,14\n5,3,18\n6,3,20", &o).unwrap();
        // Same fit as the classic sqft/rooms→price example: 2 + 2.333333·c1 + 1.333333·c2
        assert!(out.contains("(Intercept)  2\n"), "{out}");
        assert!(out.contains("2.333333"), "{out}");
        assert!(out.contains("1.333333"), "{out}");
    }

    #[test]
    fn header_and_target_by_name() {
        let o = Options {
            target: "price".into(),
            ..opts("linear")
        };
        let out = run(
            "sqft,rooms,price\n1,1,6\n2,1,8\n3,2,11\n4,2,14\n5,3,18\n6,3,20",
            &o,
        )
        .unwrap();
        assert!(out.contains("Target: price"), "{out}");
        assert!(out.contains("Predictors (2): sqft, rooms"), "{out}");
    }

    #[test]
    fn feature_subset_and_index_target() {
        let o = Options {
            target: "3".into(),
            features: "1".into(),
            ..opts("linear")
        };
        let out = run(
            "sqft,rooms,price\n1,1,6\n2,1,8\n3,2,11\n4,2,14\n5,3,18\n6,3,20",
            &o,
        )
        .unwrap();
        assert!(out.contains("Predictors (1): sqft"), "{out}");
    }

    #[test]
    fn ridge_shrinks_coefficients_toward_zero() {
        let data = "x,y\n1,3\n2,5\n3,7\n4,9\n5,11";
        let strong = Options {
            alpha: 100.0,
            decimals: 6,
            ..opts("ridge")
        };
        let weak = Options {
            alpha: 0.0,
            decimals: 6,
            ..opts("ridge")
        };
        let big = run(data, &weak).unwrap();
        let small = run(data, &strong).unwrap();
        let slope = |s: &str| -> f64 {
            s.lines()
                .find(|l| l.trim_start().starts_with("x "))
                .unwrap()
                .split_whitespace()
                .nth(1)
                .unwrap()
                .parse()
                .unwrap()
        };
        assert!((slope(&big) - 2.0).abs() < 1e-9, "{big}");
        assert!(slope(&small) < 1.9 && slope(&small) > 0.0, "{small}");
    }

    #[test]
    fn ridge_handles_collinear_columns_that_break_ols() {
        // x2 = 2·x1 exactly → the OLS normal equations are singular.
        let data = "x1,x2,y\n1,2,3\n2,4,5\n3,6,7\n4,8,9";
        let err = run(data, &opts("linear")).unwrap_err();
        assert!(err.contains("collinear"), "{err}");
        let out = run(data, &opts("ridge")).unwrap();
        assert!(out.contains("Ridge regression"), "{out}");
    }

    #[test]
    fn random_forest_fits_and_reports_importance_and_oob() {
        let mut data = String::from("a,b,y\n");
        for i in 0..60 {
            let a = i as f64;
            let b = (i % 7) as f64;
            // y depends on `a` only — importance should favour it.
            data.push_str(&format!("{a},{b},{}\n", 3.0 * a + 1.0));
        }
        let o = Options {
            trees: 40,
            ..opts("random_forest")
        };
        let out = run(&data, &o).unwrap();
        assert!(out.contains("Random forest regression (40 trees"), "{out}");
        assert!(out.contains("Out-of-bag"), "{out}");
        assert!(out.contains("Feature importance"), "{out}");
        let imp_a = out
            .lines()
            .skip_while(|l| !l.starts_with("Feature importance"))
            .nth(1)
            .unwrap();
        assert!(imp_a.trim_start().starts_with('a'), "{out}");
        let r2: f64 = out
            .lines()
            .find(|l| l.trim_start().starts_with("R²"))
            .unwrap()
            .split_whitespace()
            .nth(1)
            .unwrap()
            .parse()
            .unwrap();
        assert!(r2 > 0.9, "training R² should be high, got {r2}\n{out}");
    }

    #[test]
    fn test_split_and_cross_validation_report_extra_blocks() {
        let mut data = String::from("x,y\n");
        for i in 0..40 {
            data.push_str(&format!("{i},{}\n", 2 * i + 1));
        }
        let o = Options {
            test_split: 0.25,
            cv_folds: 5,
            ..opts("linear")
        };
        let out = run(&data, &o).unwrap();
        assert!(out.contains("train 30, test 10"), "{out}");
        assert!(out.contains("Held-out test (n = 10)"), "{out}");
        assert!(out.contains("5-fold cross-validation"), "{out}");
    }

    #[test]
    fn json_and_csv_formats() {
        let data = "x,y\n1,3\n2,5\n3,7\n4,9";
        let j = run(
            data,
            &Options {
                format: "json".into(),
                ..opts("linear")
            },
        )
        .unwrap();
        assert!(j.starts_with("{\"model\":\"linear\""), "{j}");
        assert!(j.contains("\"coefficients\":{\"x\":2}"), "{j}");
        let c = run(
            data,
            &Options {
                format: "csv".into(),
                ..opts("linear")
            },
        )
        .unwrap();
        assert!(c.starts_with("section,name,value\n"), "{c}");
        assert!(c.contains("coefficient,x,2"), "{c}");
    }

    #[test]
    fn missing_values_drop_rows_and_are_reported() {
        let data = "x,y\n1,3\n2,\n3,7\n4,9\n5,NA";
        let out = run(data, &opts("linear")).unwrap();
        assert!(
            out.contains("Rows: 3 used (2 dropped for missing values)"),
            "{out}"
        );
    }

    #[test]
    fn determinism_same_seed_same_forest() {
        let mut data = String::from("a,y\n");
        for i in 0..30 {
            data.push_str(&format!("{i},{}\n", (i * i) as f64));
        }
        let o = Options {
            trees: 25,
            ..opts("random_forest")
        };
        assert_eq!(run(&data, &o).unwrap(), run(&data, &o).unwrap());
        let other = Options {
            seed: 7,
            ..o.clone()
        };
        assert_ne!(run(&data, &o).unwrap(), run(&data, &other).unwrap());
    }

    #[test]
    fn error_non_numeric_cell() {
        let err = run("x,y\n1,3\n2,oops\n3,7", &opts("linear")).unwrap_err();
        assert!(err.contains("row 2, column 'y'"), "{err}");
        assert!(err.contains("not a number"), "{err}");
    }

    #[test]
    fn error_unknown_target_column() {
        let o = Options {
            target: "profit".into(),
            ..opts("linear")
        };
        let err = run("x,y\n1,3\n2,5\n3,7", &o).unwrap_err();
        assert!(err.contains("target column 'profit' not found"), "{err}");
        assert!(err.contains("x, y"), "{err}");
    }

    #[test]
    fn error_bad_model_and_bad_format() {
        assert!(run("x,y\n1,2\n2,4\n3,6", &opts("svm"))
            .unwrap_err()
            .contains("model must be"));
        let err = run(
            "x,y\n1,2\n2,4\n3,6",
            &Options {
                format: "xml".into(),
                ..opts("linear")
            },
        )
        .unwrap_err();
        assert!(err.contains("format must be"), "{err}");
    }

    #[test]
    fn error_too_few_rows_and_single_column() {
        let err = run("x,y\n1,3\n2,5", &opts("linear")).unwrap_err();
        assert!(err.contains("at least 3 usable rows"), "{err}");
        let err = run("1\n2\n3\n4", &opts("linear")).unwrap_err();
        assert!(err.contains("at least 2 columns"), "{err}");
    }

    #[test]
    fn error_forest_work_budget() {
        let mut data = String::from("x,y\n");
        for i in 0..4000 {
            data.push_str(&format!("{i},{}\n", i * 2));
        }
        let o = Options {
            trees: 300,
            ..opts("random_forest")
        };
        let err = run(&data, &o).unwrap_err();
        assert!(err.contains("work budget exceeded"), "{err}");
    }

    #[test]
    fn tab_and_semicolon_delimiters_parse() {
        let out = run("x\ty\n1\t3\n2\t5\n3\t7", &opts("linear")).unwrap();
        assert!(out.contains("Equation: y = 1 + 2·x"), "{out}");
        let out = run("x;y\n1;3\n2;5\n3;7", &opts("linear")).unwrap();
        assert!(out.contains("Equation: y = 1 + 2·x"), "{out}");
    }
}
