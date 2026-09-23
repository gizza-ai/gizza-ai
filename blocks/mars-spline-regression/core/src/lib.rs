//! mars-spline-regression core — pure compute, shared by the chat skill block, the CLI
//! and the web page. No wafer/wasm-bindgen deps.
//!
//! Fits a MARS model (Multivariate Adaptive Regression Splines, Friedman 1991) to a pasted
//! numeric table: a forward pass grows a basis of hinge functions `h(x - t)` / `h(t - x)`
//! whose knots `t` are chosen from the data, and a backward pass prunes the basis back to
//! whichever sub-model minimises the GCV score. The result is an explicit piecewise-linear
//! equation, a term table with coefficients, fit statistics (RSS, R², GCV, GRSq, RMSE, MAE),
//! variable importance and optional predictions for new rows.
//!
//! Everything is deterministic — no sampling, no randomness — so the same input always
//! produces byte-identical output.

use serde::Serialize;

/// Hard caps — keep a pasted table inside what a browser tab can chew through.
pub const MAX_ROWS: usize = 5_000;
pub const MAX_COLS: usize = 50;
pub const MAX_PREDICT_ROWS: usize = 1_000;

/// Candidate-scoring budget for the forward pass, in multiply-add units. Knot candidates
/// are thinned evenly until one forward iteration fits inside this budget, so wide tables
/// and long tables stay responsive instead of hanging the tab.
const MAX_WORK: usize = 60_000_000;
/// Never consider more than this many knots for a single (parent term, variable) pair.
const MAX_KNOTS_PER_PAIR: usize = 256;

/// Every knob the tool exposes. `Default` mirrors the descriptor defaults exactly.
#[derive(Clone, Debug)]
pub struct Options {
    pub target: String,
    pub features: String,
    pub max_terms: u32,
    pub max_degree: u32,
    pub penalty: f64,
    pub prune: bool,
    pub nprune: u32,
    pub minspan: u32,
    pub endspan: u32,
    pub thresh: f64,
    pub allow_linear: bool,
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
            max_terms: 21,
            max_degree: 1,
            penalty: 3.0,
            prune: true,
            nprune: 0,
            minspan: 0,
            endspan: 0,
            thresh: 0.001,
            allow_linear: true,
            predict: String::new(),
            header: "auto".into(),
            decimals: 4,
            format: "text".into(),
        }
    }
}

// ---------------------------------------------------------------------- parsing ---

fn is_missing(tok: &str) -> bool {
    matches!(
        tok.trim().to_ascii_lowercase().as_str(),
        "" | "na" | "n/a" | "nan" | "null" | "none" | "-" | "?" | "."
    )
}

fn looks_numeric(tok: &str) -> bool {
    tok.trim().replace('_', "").parse::<f64>().is_ok()
}

fn parse_cell(tok: &str) -> Option<f64> {
    tok.trim().replace('_', "").parse::<f64>().ok()
}

/// Pick the column delimiter from a line: whichever of comma / tab / semicolon / pipe
/// occurs most often, else `None`, meaning "split on runs of whitespace".
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

fn split_row(line: &str, delim: Option<char>) -> Vec<String> {
    match delim {
        Some(d) => line.split(d).map(|s| s.trim().to_string()).collect(),
        None => line.split_whitespace().map(|s| s.to_string()).collect(),
    }
}

#[derive(Clone, Copy, PartialEq)]
enum HeaderMode {
    Auto,
    Yes,
    No,
}

fn parse_header_mode(header: &str) -> Result<HeaderMode, String> {
    match header.trim().to_ascii_lowercase().as_str() {
        "" | "auto" => Ok(HeaderMode::Auto),
        "yes" | "true" | "1" => Ok(HeaderMode::Yes),
        "no" | "false" | "0" => Ok(HeaderMode::No),
        other => Err(format!(
            "header must be 'auto', 'yes' or 'no' (got '{other}')"
        )),
    }
}

struct Table {
    labels: Vec<String>,
    /// `true` when the labels came from a real header row rather than being generated.
    labelled: bool,
    rows: Vec<Vec<f64>>,
}

fn parse_table(data: &str, mode: HeaderMode) -> Result<Table, String> {
    let lines: Vec<&str> = data
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect();
    if lines.is_empty() {
        return Err("no data — paste a numeric table with one row per line".to_string());
    }
    let delim = detect_delim(lines[0]);
    let first = split_row(lines[0], delim);
    if first.len() < 2 {
        return Err(
            "each row needs at least two columns (one or more predictors plus the target); \
             separate them with commas, tabs, semicolons, pipes or spaces"
                .to_string(),
        );
    }
    if first.len() > MAX_COLS {
        return Err(format!(
            "too many columns: {} (maximum {MAX_COLS})",
            first.len()
        ));
    }
    let has_header = match mode {
        HeaderMode::Yes => true,
        HeaderMode::No => false,
        HeaderMode::Auto => first.iter().any(|t| !looks_numeric(t)),
    };
    let ncol = first.len();
    let labels: Vec<String> = if has_header {
        first
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let t = t.trim();
                if t.is_empty() {
                    format!("x{}", i + 1)
                } else {
                    t.to_string()
                }
            })
            .collect()
    } else {
        (1..=ncol).map(|i| format!("x{i}")).collect()
    };
    let start = usize::from(has_header);
    let mut rows: Vec<Vec<f64>> = Vec::with_capacity(lines.len().saturating_sub(start));
    for (li, line) in lines.iter().enumerate().skip(start) {
        let toks = split_row(line, delim);
        if toks.len() != ncol {
            return Err(format!(
                "row {} has {} columns but the first row has {ncol} — every row needs the same number of columns",
                li + 1,
                toks.len()
            ));
        }
        let mut row = Vec::with_capacity(ncol);
        for (ci, tok) in toks.iter().enumerate() {
            match parse_cell(tok) {
                Some(v) if v.is_finite() => row.push(v),
                _ => {
                    return Err(if is_missing(tok) {
                        format!(
                            "row {} column {} ({}) is empty or missing — this tool needs a complete numeric table; remove or fill the row",
                            li + 1,
                            ci + 1,
                            labels[ci]
                        )
                    } else {
                        format!(
                            "row {} column {} ({}) is not a finite number: '{}'",
                            li + 1,
                            ci + 1,
                            labels[ci],
                            tok.trim()
                        )
                    })
                }
            }
        }
        rows.push(row);
        if rows.len() > MAX_ROWS {
            return Err(format!("too many rows: maximum {MAX_ROWS}"));
        }
    }
    if rows.len() < 3 {
        return Err(format!(
            "need at least 3 data rows to fit a model (got {})",
            rows.len()
        ));
    }
    Ok(Table {
        labels,
        labelled: has_header,
        rows,
    })
}

/// Resolve a column selector: `last`, `first`, a 1-based index, or a (case-insensitive) label.
fn resolve_column(sel: &str, labels: &[String], what: &str) -> Result<usize, String> {
    let s = sel.trim();
    let lower = s.to_ascii_lowercase();
    if lower == "last" {
        return Ok(labels.len() - 1);
    }
    if lower == "first" {
        return Ok(0);
    }
    if let Ok(i) = s.parse::<i64>() {
        if i >= 1 && (i as usize) <= labels.len() {
            return Ok(i as usize - 1);
        }
        return Err(format!(
            "{what} column index {i} is out of range — the table has {} columns",
            labels.len()
        ));
    }
    if let Some(i) = labels.iter().position(|l| l.eq_ignore_ascii_case(s)) {
        return Ok(i);
    }
    Err(format!(
        "{what} column '{s}' not found — available columns: {}",
        labels.join(", ")
    ))
}

// ------------------------------------------------------------------ model terms ---

/// One factor of a basis function: a hinge on a variable, or a plain linear term.
#[derive(Clone, Copy, Debug, Serialize)]
struct Factor {
    var: usize,
    knot: f64,
    /// `1` = `h(x - knot)`, `-1` = `h(knot - x)`, `0` = plain linear `x` (no knot).
    dir: i8,
}

impl Factor {
    fn eval(&self, x: f64) -> f64 {
        match self.dir {
            1 => (x - self.knot).max(0.0),
            -1 => (self.knot - x).max(0.0),
            _ => x,
        }
    }
    fn name(&self, labels: &[String], decimals: usize) -> String {
        let v = &labels[self.var];
        match self.dir {
            1 => format!("h({v} - {})", fmt_num(self.knot, decimals)),
            -1 => format!("h({} - {v})", fmt_num(self.knot, decimals)),
            _ => v.clone(),
        }
    }
}

/// A basis function: the product of its factors. No factors = the intercept.
#[derive(Clone, Debug, Default)]
struct Term {
    factors: Vec<Factor>,
}

impl Term {
    fn eval(&self, row: &[f64]) -> f64 {
        let mut v = 1.0;
        for f in &self.factors {
            v *= f.eval(row[f.var]);
            if v == 0.0 {
                return 0.0;
            }
        }
        v
    }
    fn name(&self, labels: &[String], decimals: usize) -> String {
        if self.factors.is_empty() {
            return "(Intercept)".to_string();
        }
        self.factors
            .iter()
            .map(|f| f.name(labels, decimals))
            .collect::<Vec<_>>()
            .join(" * ")
    }
    fn uses(&self, var: usize) -> bool {
        self.factors.iter().any(|f| f.var == var)
    }
    fn degree(&self) -> usize {
        self.factors.len()
    }
}

// --------------------------------------------------------------- linear algebra ---

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// Orthogonalise `c` against the orthonormal `basis` and normalise it. Returns `None` when
/// the column is (numerically) already in the span — i.e. it would add no information.
fn orthonormalize(basis: &[Vec<f64>], extra: Option<&Vec<f64>>, c: &[f64]) -> Option<Vec<f64>> {
    let mut v = c.to_vec();
    let norm0 = dot(&v, &v).sqrt();
    if !norm0.is_finite() || norm0 <= 0.0 {
        return None;
    }
    let project = |v: &mut Vec<f64>| {
        for b in basis.iter().chain(extra) {
            let d = dot(b, v);
            for (vi, bi) in v.iter_mut().zip(b) {
                *vi -= d * bi;
            }
        }
    };
    project(&mut v);
    let mut n = dot(&v, &v).sqrt();
    // Classical Gram-Schmidt loses orthogonality when the column is nearly dependent;
    // re-project once in exactly that case rather than paying for it on every candidate.
    if n < 0.1 * norm0 {
        project(&mut v);
        n = dot(&v, &v).sqrt();
    }
    if !n.is_finite() || n < 1e-8 * norm0 {
        return None;
    }
    for x in v.iter_mut() {
        *x /= n;
    }
    Some(v)
}

/// Least squares by modified Gram-Schmidt QR. Returns the coefficients and the RSS, or
/// `None` if the columns are rank deficient.
fn lstsq(cols: &[&Vec<f64>], y: &[f64]) -> Option<(Vec<f64>, f64)> {
    let p = cols.len();
    let n = y.len();
    if p == 0 || p > n {
        return None;
    }
    let mut q: Vec<Vec<f64>> = Vec::with_capacity(p);
    let mut r = vec![vec![0.0f64; p]; p];
    let mut rhs = vec![0.0f64; p];
    for (j, col) in cols.iter().enumerate() {
        let mut v = (*col).clone();
        let norm0 = dot(&v, &v).sqrt();
        if !norm0.is_finite() || norm0 <= 0.0 {
            return None;
        }
        for (i, qi) in q.iter().enumerate() {
            let d = dot(qi, &v);
            r[i][j] = d;
            for (vi, qv) in v.iter_mut().zip(qi) {
                *vi -= d * qv;
            }
        }
        let nrm = dot(&v, &v).sqrt();
        if !nrm.is_finite() || nrm < 1e-10 * norm0 {
            return None;
        }
        r[j][j] = nrm;
        for x in v.iter_mut() {
            *x /= nrm;
        }
        rhs[j] = dot(&v, y);
        q.push(v);
    }
    // Back-substitute R b = rhs.
    let mut b = vec![0.0f64; p];
    for j in (0..p).rev() {
        let mut s = rhs[j];
        for k in (j + 1)..p {
            s -= r[j][k] * b[k];
        }
        b[j] = s / r[j][j];
    }
    let rss = (dot(y, y) - rhs.iter().map(|v| v * v).sum::<f64>()).max(0.0);
    Some((b, rss))
}

/// Friedman's GCV: the training MSE inflated by an effective-parameter penalty.
/// `c(M) = M + penalty * (M - 1) / 2`, matching the reference implementations.
fn gcv(rss: f64, n: usize, terms: usize, penalty: f64) -> f64 {
    let n = n as f64;
    let c = terms as f64 + penalty * (terms as f64 - 1.0) / 2.0;
    let denom = 1.0 - c / n;
    if denom <= 0.0 {
        return f64::INFINITY;
    }
    (rss / n) / (denom * denom)
}

/// Friedman's automatic end span: how many extreme values of each variable are ineligible
/// as knots.
fn auto_endspan(nvars: usize, n: usize) -> usize {
    let v = 3.0 - (0.05 / nvars.max(1) as f64).log2();
    let friedman = (v.round().max(1.0) as usize).max(1);
    // Friedman's span assumes a reasonably long table. On a short one it can make EVERY
    // observation ineligible as a knot (a knot needs more than `2 * endspan` rows), which
    // silently degrades MARS to a plain linear fit. Cap the automatic value so at least
    // half the rows stay eligible; an explicit `endspan` is always honoured as given.
    friedman.min((n.saturating_sub(1) / 4).max(1))
}

/// Friedman's automatic minimum span: how many observations must sit between two knots.
fn auto_minspan(n: usize, nvars: usize) -> usize {
    let a = -(1.0 / (nvars.max(1) as f64 * n as f64)) * (1.0f64 - 0.05).ln();
    if a <= 0.0 {
        return 1;
    }
    ((-a.log2()) / 2.5).round().max(1.0) as usize
}

// ------------------------------------------------------------------- the fitting ---

#[derive(Debug, Serialize)]
pub struct TermOut {
    pub index: usize,
    pub name: String,
    pub coefficient: f64,
    pub degree: usize,
}

#[derive(Debug, Serialize)]
pub struct KnotOut {
    pub variable: String,
    pub knots: Vec<f64>,
}

#[derive(Debug, Serialize)]
pub struct ImportanceOut {
    pub variable: String,
    pub importance: f64,
    pub terms: usize,
}

#[derive(Debug, Serialize)]
pub struct PointOut {
    pub row: usize,
    pub actual: f64,
    pub fitted: f64,
    pub residual: f64,
}

#[derive(Debug, Serialize)]
pub struct PredictionOut {
    pub row: usize,
    pub inputs: Vec<f64>,
    pub predicted: f64,
}

#[derive(Debug, Serialize)]
pub struct Fit {
    pub equation: String,
    pub target: String,
    pub features: Vec<String>,
    pub observations: usize,
    pub max_degree: usize,
    pub model_degree: usize,
    pub forward_terms: usize,
    pub kept_terms: usize,
    pub pruned: bool,
    pub predictors_used: usize,
    pub knot_candidates_per_variable: usize,
    pub minspan: usize,
    pub endspan: usize,
    pub terms: Vec<TermOut>,
    pub knots: Vec<KnotOut>,
    pub importance: Vec<ImportanceOut>,
    pub rss: f64,
    pub r_squared: f64,
    pub gcv: f64,
    pub grsq: f64,
    pub rmse: f64,
    pub mae: f64,
    pub residual_min: f64,
    pub residual_median: f64,
    pub residual_max: f64,
    pub points: Vec<PointOut>,
    pub predictions: Vec<PredictionOut>,
}

/// Build the candidate knot list for `var`, restricted to rows where the parent basis is
/// non-zero and thinned by end span, minimum span and the work budget.
fn candidate_knots(
    x: &[Vec<f64>],
    var: usize,
    parent_col: &[f64],
    endspan: usize,
    minspan: usize,
    max_knots: usize,
) -> Vec<f64> {
    let mut vals: Vec<f64> = x
        .iter()
        .zip(parent_col)
        .filter(|(_, p)| **p != 0.0)
        .map(|(row, _)| row[var])
        .collect();
    if vals.len() < 3 {
        return Vec::new();
    }
    vals.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    // End span: the most extreme observations are never eligible as knots.
    if vals.len() <= 2 * endspan + 1 {
        return Vec::new();
    }
    let slice = &vals[endspan..vals.len() - endspan];
    // Minimum span: keep every `minspan`-th eligible observation, de-duplicated.
    let mut picked: Vec<f64> = Vec::new();
    let step = minspan.max(1);
    let mut i = 0;
    while i < slice.len() {
        let v = slice[i];
        if picked.last().map(|l| *l != v).unwrap_or(true) {
            picked.push(v);
        }
        i += step;
    }
    if picked.len() <= max_knots || max_knots == 0 {
        return picked;
    }
    // Still too many: thin evenly so the retained knots span the same range.
    let mut out = Vec::with_capacity(max_knots);
    for k in 0..max_knots {
        let idx = (k * (picked.len() - 1)) / (max_knots - 1).max(1);
        let v = picked[idx];
        if out.last().map(|l| *l != v).unwrap_or(true) {
            out.push(v);
        }
    }
    out
}

struct Candidate {
    gain: f64,
    parent: usize,
    var: usize,
    knot: f64,
    /// `true` for a hinge pair, `false` for a single plain linear term.
    pair: bool,
}

/// The forward pass: grow the basis one (pair of) hinge term(s) at a time, always taking the
/// candidate that reduces the residual sum of squares the most.
#[allow(clippy::too_many_arguments)]
fn forward_pass(
    x: &[Vec<f64>],
    y: &[f64],
    feats: &[usize],
    max_terms: usize,
    max_degree: usize,
    thresh: f64,
    allow_linear: bool,
    endspan: usize,
    minspan: usize,
) -> (Vec<Term>, Vec<Vec<f64>>, usize) {
    let n = y.len();
    let mut terms: Vec<Term> = vec![Term::default()];
    let mut cols: Vec<Vec<f64>> = vec![vec![1.0; n]];
    let mut basis: Vec<Vec<f64>> = Vec::new();
    let mut resid = y.to_vec();
    let sst = {
        let mean = y.iter().sum::<f64>() / n as f64;
        y.iter().map(|v| (v - mean) * (v - mean)).sum::<f64>()
    };

    // Seat the intercept first so every later gain is measured against a centred residual.
    if let Some(q0) = orthonormalize(&basis, None, &cols[0]) {
        let d = dot(&q0, &resid);
        for (r, q) in resid.iter_mut().zip(&q0) {
            *r -= d * q;
        }
        basis.push(q0);
    }

    let mut knots_per_var = 0usize;
    while terms.len() + 1 <= max_terms {
        let parents: Vec<usize> = (0..terms.len())
            .filter(|&i| terms[i].degree() < max_degree)
            .collect();
        if parents.is_empty() {
            break;
        }
        // Work budget: one candidate costs roughly 2 * |basis| * n multiply-adds.
        let per_cand = (2 * basis.len().max(1) * n).max(1);
        let pairs = (parents.len() * feats.len()).max(1);
        let max_knots = (MAX_WORK / per_cand / pairs)
            .clamp(1, MAX_KNOTS_PER_PAIR)
            .max(1);
        knots_per_var = knots_per_var.max(max_knots);

        let mut best: Option<Candidate> = None;
        for &p in &parents {
            for &v in feats {
                if terms[p].uses(v) {
                    continue; // a variable may appear at most once per basis function
                }
                if allow_linear {
                    let mut c = vec![0.0; n];
                    for (i, ci) in c.iter_mut().enumerate() {
                        *ci = cols[p][i] * x[i][v];
                    }
                    if let Some(q) = orthonormalize(&basis, None, &c) {
                        let g = dot(&q, &resid).powi(2);
                        if best.as_ref().map(|b| g > b.gain).unwrap_or(true) {
                            best = Some(Candidate {
                                gain: g,
                                parent: p,
                                var: v,
                                knot: 0.0,
                                pair: false,
                            });
                        }
                    }
                }
                for knot in candidate_knots(x, v, &cols[p], endspan, minspan, max_knots) {
                    let mut c1 = vec![0.0; n];
                    let mut c2 = vec![0.0; n];
                    for i in 0..n {
                        let pv = cols[p][i];
                        if pv != 0.0 {
                            c1[i] = pv * (x[i][v] - knot).max(0.0);
                            c2[i] = pv * (knot - x[i][v]).max(0.0);
                        }
                    }
                    let q1 = orthonormalize(&basis, None, &c1);
                    let g1 = q1.as_ref().map(|q| dot(q, &resid).powi(2)).unwrap_or(0.0);
                    let q2 = orthonormalize(&basis, q1.as_ref(), &c2);
                    let g2 = q2.as_ref().map(|q| dot(q, &resid).powi(2)).unwrap_or(0.0);
                    if q1.is_none() && q2.is_none() {
                        continue;
                    }
                    let g = g1 + g2;
                    if best.as_ref().map(|b| g > b.gain).unwrap_or(true) {
                        best = Some(Candidate {
                            gain: g,
                            parent: p,
                            var: v,
                            knot,
                            pair: true,
                        });
                    }
                }
            }
        }

        let Some(b) = best else { break };
        // Stop once the best candidate no longer buys a meaningful R² improvement.
        if sst <= 0.0 || b.gain / sst < thresh {
            break;
        }

        let mut add: Vec<(Term, Vec<f64>)> = Vec::new();
        if b.pair {
            for dir in [1i8, -1i8] {
                let mut t = terms[b.parent].clone();
                t.factors.push(Factor {
                    var: b.var,
                    knot: b.knot,
                    dir,
                });
                let col: Vec<f64> = (0..n)
                    .map(|i| {
                        let pv = cols[b.parent][i];
                        if pv == 0.0 {
                            0.0
                        } else {
                            pv * Factor {
                                var: b.var,
                                knot: b.knot,
                                dir,
                            }
                            .eval(x[i][b.var])
                        }
                    })
                    .collect();
                add.push((t, col));
            }
        } else {
            let mut t = terms[b.parent].clone();
            t.factors.push(Factor {
                var: b.var,
                knot: 0.0,
                dir: 0,
            });
            let col: Vec<f64> = (0..n).map(|i| cols[b.parent][i] * x[i][b.var]).collect();
            add.push((t, col));
        }

        let mut added = false;
        for (t, col) in add {
            if terms.len() >= max_terms {
                break;
            }
            let Some(q) = orthonormalize(&basis, None, &col) else {
                continue; // degenerate half of the pair (e.g. a knot at the extreme) — skip it
            };
            let d = dot(&q, &resid);
            for (r, qv) in resid.iter_mut().zip(&q) {
                *r -= d * qv;
            }
            basis.push(q);
            terms.push(t);
            cols.push(col);
            added = true;
        }
        if !added {
            break;
        }
    }
    (terms, cols, knots_per_var)
}

/// The backward pass: greedily drop the least useful term, and keep whichever sub-model
/// along that path has the lowest GCV (subject to `nprune`).
fn backward_pass(
    cols: &[Vec<f64>],
    y: &[f64],
    penalty: f64,
    nprune: usize,
) -> (Vec<usize>, Vec<f64>, f64) {
    let n = y.len();
    let mut current: Vec<usize> = (0..cols.len()).collect();
    let mut best: Option<(f64, Vec<usize>, Vec<f64>, f64)> = None;

    let consider = |set: &[usize], best: &mut Option<(f64, Vec<usize>, Vec<f64>, f64)>| {
        if nprune > 0 && set.len() > nprune {
            return;
        }
        let refs: Vec<&Vec<f64>> = set.iter().map(|&i| &cols[i]).collect();
        if let Some((b, rss)) = lstsq(&refs, y) {
            let g = gcv(rss, n, set.len(), penalty);
            if best.as_ref().map(|(bg, ..)| g < *bg).unwrap_or(true) {
                *best = Some((g, set.to_vec(), b, rss));
            }
        }
    };

    consider(&current, &mut best);
    while current.len() > 1 {
        let mut drop: Option<(f64, usize)> = None;
        for pos in 1..current.len() {
            // index 0 is the intercept and is never dropped
            let mut trial = current.clone();
            trial.remove(pos);
            let refs: Vec<&Vec<f64>> = trial.iter().map(|&i| &cols[i]).collect();
            if let Some((_, rss)) = lstsq(&refs, y) {
                if drop.map(|(r, _)| rss < r).unwrap_or(true) {
                    drop = Some((rss, pos));
                }
            }
        }
        let Some((_, pos)) = drop else { break };
        current.remove(pos);
        consider(&current, &mut best);
    }

    match best {
        Some((g, set, coefs, rss)) => (set, coefs, {
            let _ = g;
            rss
        }),
        // Every sub-model was rank deficient — fall back to the intercept alone.
        None => {
            let mean = y.iter().sum::<f64>() / n as f64;
            let rss = y.iter().map(|v| (v - mean) * (v - mean)).sum::<f64>();
            (vec![0], vec![mean], rss)
        }
    }
}

fn median(v: &mut [f64]) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let m = v.len() / 2;
    if v.len() % 2 == 0 {
        (v[m - 1] + v[m]) / 2.0
    } else {
        v[m]
    }
}

fn parse_predict_rows(
    predict: &str,
    nfeat: usize,
    mode: HeaderMode,
) -> Result<Vec<Vec<f64>>, String> {
    let lines: Vec<&str> = predict
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect();
    if lines.is_empty() {
        return Ok(Vec::new());
    }
    let delim = detect_delim(lines[0]);
    let first = split_row(lines[0], delim);
    let skip_header = match mode {
        HeaderMode::Yes => first.iter().any(|t| !looks_numeric(t)),
        HeaderMode::Auto => first.iter().any(|t| !looks_numeric(t)),
        HeaderMode::No => false,
    };
    let mut out = Vec::new();
    for (li, line) in lines.iter().enumerate().skip(usize::from(skip_header)) {
        let toks = split_row(line, delim);
        if toks.len() != nfeat {
            return Err(format!(
                "predict row {} has {} value(s) but the model uses {nfeat} feature(s) — give one value per feature, in feature order",
                li + 1,
                toks.len()
            ));
        }
        let mut row = Vec::with_capacity(nfeat);
        for tok in &toks {
            match parse_cell(tok) {
                Some(v) if v.is_finite() => row.push(v),
                _ => {
                    return Err(format!(
                        "predict row {} contains a value that is not a finite number: '{}'",
                        li + 1,
                        tok.trim()
                    ))
                }
            }
        }
        out.push(row);
        if out.len() > MAX_PREDICT_ROWS {
            return Err(format!(
                "too many predict rows: maximum {MAX_PREDICT_ROWS}"
            ));
        }
    }
    Ok(out)
}

/// Fit the model and assemble every reported number.
pub fn fit(data: &str, o: &Options) -> Result<Fit, String> {
    if o.max_terms < 2 {
        return Err("max_terms must be at least 2".to_string());
    }
    if o.max_degree < 1 {
        return Err("max_degree must be at least 1".to_string());
    }
    if !(0.0..=100.0).contains(&o.penalty) || !o.penalty.is_finite() {
        return Err("penalty must be between 0 and 100".to_string());
    }
    if !(0.0..=1.0).contains(&o.thresh) || !o.thresh.is_finite() {
        return Err("thresh must be between 0 and 1".to_string());
    }
    let mode = parse_header_mode(&o.header)?;
    let table = parse_table(data, mode)?;
    let decimals = o.decimals.min(12) as usize;

    let target = resolve_column(
        if o.target.trim().is_empty() {
            "last"
        } else {
            &o.target
        },
        &table.labels,
        "target",
    )?;
    let feats: Vec<usize> = if o.features.trim().is_empty() {
        (0..table.labels.len()).filter(|&i| i != target).collect()
    } else {
        let mut v = Vec::new();
        for sel in o
            .features
            .split([',', ';', '\n'])
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            let i = resolve_column(sel, &table.labels, "feature")?;
            if i == target {
                return Err(format!(
                    "feature '{}' is also the target column — pick different columns",
                    table.labels[i]
                ));
            }
            if !v.contains(&i) {
                v.push(i);
            }
        }
        if v.is_empty() {
            return Err("features listed no usable columns".to_string());
        }
        v
    };
    if feats.is_empty() {
        return Err("the table needs at least one predictor column besides the target".to_string());
    }

    let n = table.rows.len();
    let x: Vec<Vec<f64>> = table.rows.clone();
    let y: Vec<f64> = table.rows.iter().map(|r| r[target]).collect();
    let mean_y = y.iter().sum::<f64>() / n as f64;
    let sst: f64 = y.iter().map(|v| (v - mean_y) * (v - mean_y)).sum();
    if sst <= 0.0 {
        return Err(format!(
            "the target column '{}' is constant — there is nothing to model",
            table.labels[target]
        ));
    }

    let endspan = if o.endspan == 0 {
        auto_endspan(feats.len(), n)
    } else {
        o.endspan as usize
    };
    let minspan = if o.minspan == 0 {
        auto_minspan(n, feats.len())
    } else {
        o.minspan as usize
    };
    // Cap the forward pass at what the data can actually support.
    let max_terms = (o.max_terms as usize).min(n.saturating_sub(1)).max(2);
    let max_degree = (o.max_degree as usize).min(feats.len());

    let (terms, cols, knots_per_var) = forward_pass(
        &x,
        &y,
        &feats,
        max_terms,
        max_degree,
        o.thresh,
        o.allow_linear,
        endspan,
        minspan,
    );
    let forward_terms = terms.len();

    let nprune = o.nprune as usize;
    let (kept, coefs, rss) = if o.prune {
        backward_pass(&cols, &y, o.penalty, nprune)
    } else {
        let set: Vec<usize> = if nprune > 0 {
            (0..cols.len().min(nprune)).collect()
        } else {
            (0..cols.len()).collect()
        };
        let refs: Vec<&Vec<f64>> = set.iter().map(|&i| &cols[i]).collect();
        match lstsq(&refs, &y) {
            Some((b, rss)) => (set, b, rss),
            None => (vec![0], vec![mean_y], sst),
        }
    };

    // Labels: features keep their column names, the target names the left-hand side.
    let labels = table.labels.clone();
    let y_label = if table.labelled {
        labels[target].clone()
    } else {
        "y".to_string()
    };

    let kept_terms: Vec<&Term> = kept.iter().map(|&i| &terms[i]).collect();
    let term_out: Vec<TermOut> = kept_terms
        .iter()
        .zip(&coefs)
        .enumerate()
        .map(|(i, (t, c))| TermOut {
            index: i,
            name: t.name(&labels, decimals),
            coefficient: *c,
            degree: t.degree(),
        })
        .collect();

    // Equation.
    let mut equation = format!("{y_label} = {}", fmt_num(coefs[0], decimals));
    for (t, c) in kept_terms.iter().zip(&coefs).skip(1) {
        equation.push_str(&format!(
            "\n    {} {} * {}",
            if *c < 0.0 { '-' } else { '+' },
            fmt_num(c.abs(), decimals),
            t.name(&labels, decimals)
        ));
    }

    // Fitted values and residuals.
    let fitted: Vec<f64> = (0..n)
        .map(|i| {
            kept_terms
                .iter()
                .zip(&coefs)
                .map(|(t, c)| c * t.eval(&x[i]))
                .sum()
        })
        .collect();
    let residuals: Vec<f64> = (0..n).map(|i| y[i] - fitted[i]).collect();
    let rmse = (residuals.iter().map(|r| r * r).sum::<f64>() / n as f64).sqrt();
    let mae = residuals.iter().map(|r| r.abs()).sum::<f64>() / n as f64;
    let mut sorted = residuals.clone();
    let residual_median = median(&mut sorted);

    let r_squared = 1.0 - rss / sst;
    let g = gcv(rss, n, kept.len(), o.penalty);
    let g_null = gcv(sst, n, 1, o.penalty);
    let grsq = if g_null.is_finite() && g_null > 0.0 && g.is_finite() {
        1.0 - g / g_null
    } else {
        f64::NAN
    };

    // Knots actually used, per variable.
    let mut knots: Vec<KnotOut> = Vec::new();
    for &v in &feats {
        let mut ks: Vec<f64> = kept_terms
            .iter()
            .flat_map(|t| t.factors.iter())
            .filter(|f| f.var == v && f.dir != 0)
            .map(|f| f.knot)
            .collect();
        ks.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        ks.dedup();
        if !ks.is_empty() {
            knots.push(KnotOut {
                variable: labels[v].clone(),
                knots: ks,
            });
        }
    }

    // Variable importance: how much the RSS grows when every term using the variable goes.
    let mut raw: Vec<(usize, f64, usize)> = Vec::new();
    for &v in &feats {
        let used = kept_terms.iter().filter(|t| t.uses(v)).count();
        if used == 0 {
            continue;
        }
        let subset: Vec<&Vec<f64>> = kept
            .iter()
            .filter(|&&i| !terms[i].uses(v))
            .map(|&i| &cols[i])
            .collect();
        let without = match lstsq(&subset, &y) {
            Some((_, r)) => r,
            None => sst,
        };
        raw.push((v, (without - rss).max(0.0), used));
    }
    let max_raw = raw.iter().map(|(_, r, _)| *r).fold(0.0f64, f64::max);
    raw.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let importance: Vec<ImportanceOut> = raw
        .iter()
        .map(|(v, r, t)| ImportanceOut {
            variable: labels[*v].clone(),
            importance: if max_raw > 0.0 { r / max_raw * 100.0 } else { 0.0 },
            terms: *t,
        })
        .collect();

    // Predictions for new rows.
    let predict_rows = parse_predict_rows(&o.predict, feats.len(), mode)?;
    let mut predictions = Vec::with_capacity(predict_rows.len());
    for (i, pr) in predict_rows.iter().enumerate() {
        // Scatter the feature values back into a full-width row so terms index as usual.
        let mut row = vec![0.0; labels.len()];
        for (k, &v) in feats.iter().enumerate() {
            row[v] = pr[k];
        }
        let value: f64 = kept_terms
            .iter()
            .zip(&coefs)
            .map(|(t, c)| c * t.eval(&row))
            .sum();
        predictions.push(PredictionOut {
            row: i + 1,
            inputs: pr.clone(),
            predicted: value,
        });
    }

    let predictors_used = feats
        .iter()
        .filter(|&&v| kept_terms.iter().any(|t| t.uses(v)))
        .count();
    let model_degree = kept_terms.iter().map(|t| t.degree()).max().unwrap_or(0);

    Ok(Fit {
        equation,
        target: y_label,
        features: feats.iter().map(|&v| labels[v].clone()).collect(),
        observations: n,
        max_degree,
        model_degree,
        forward_terms,
        kept_terms: kept.len(),
        pruned: o.prune,
        predictors_used,
        knot_candidates_per_variable: knots_per_var,
        minspan,
        endspan,
        terms: term_out,
        knots,
        importance,
        rss,
        r_squared,
        gcv: g,
        grsq,
        rmse,
        mae,
        residual_min: residuals.iter().cloned().fold(f64::INFINITY, f64::min),
        residual_median,
        residual_max: residuals.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
        points: (0..n)
            .map(|i| PointOut {
                row: i + 1,
                actual: y[i],
                fitted: fitted[i],
                residual: residuals[i],
            })
            .collect(),
        predictions,
    })
}

// ------------------------------------------------------------------- formatting ---

fn fmt_num(v: f64, decimals: usize) -> String {
    if v.is_nan() {
        return "n/a".to_string();
    }
    if v.is_infinite() {
        return if v > 0.0 { "∞" } else { "-∞" }.to_string();
    }
    let s = format!("{v:.decimals$}");
    // Avoid printing a signed zero like "-0.0000".
    if s.starts_with('-') && s[1..].chars().all(|c| c == '0' || c == '.') {
        return s[1..].to_string();
    }
    s
}

fn render_text(f: &Fit, d: usize) -> String {
    let mut out = String::new();
    out.push_str(&f.equation);
    out.push_str("\n\nModel\n");
    out.push_str(&format!(
        "  terms kept        {} of {} from the forward pass{}\n",
        f.kept_terms,
        f.forward_terms,
        if f.pruned {
            " (backward pruning by GCV)"
        } else {
            " (pruning off)"
        }
    ));
    out.push_str(&format!(
        "  interaction       degree {} used, {} allowed\n",
        f.model_degree.max(1),
        f.max_degree
    ));
    out.push_str(&format!("  observations      {}\n", f.observations));
    out.push_str(&format!(
        "  predictors used   {} of {}\n",
        f.predictors_used,
        f.features.len()
    ));
    out.push_str(&format!(
        "  knot search       min span {}, end span {}, up to {} candidate knots per variable\n",
        f.minspan, f.endspan, f.knot_candidates_per_variable
    ));

    out.push_str("\nTerms\n");
    let width = f
        .terms
        .iter()
        .map(|t| t.name.chars().count())
        .max()
        .unwrap_or(4)
        .max(4);
    out.push_str(&format!("  {:<width$}  coefficient\n", "term"));
    for t in &f.terms {
        out.push_str(&format!(
            "  {:<width$}  {}\n",
            t.name,
            fmt_num(t.coefficient, d)
        ));
    }

    out.push_str("\nFit\n");
    out.push_str(&format!("  RSS               {}\n", fmt_num(f.rss, d)));
    out.push_str(&format!("  R²                {}\n", fmt_num(f.r_squared, d)));
    out.push_str(&format!("  GCV               {}\n", fmt_num(f.gcv, d)));
    out.push_str(&format!("  GRSq              {}\n", fmt_num(f.grsq, d)));
    out.push_str(&format!("  RMSE              {}\n", fmt_num(f.rmse, d)));
    out.push_str(&format!("  MAE               {}\n", fmt_num(f.mae, d)));

    if !f.knots.is_empty() {
        out.push_str("\nKnots\n");
        for k in &f.knots {
            out.push_str(&format!(
                "  {}  {}\n",
                k.variable,
                k.knots
                    .iter()
                    .map(|v| fmt_num(*v, d))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
    }

    if !f.importance.is_empty() {
        out.push_str("\nVariable importance (RSS drop, best = 100)\n");
        for i in &f.importance {
            out.push_str(&format!(
                "  {}  {}  ({} term{})\n",
                i.variable,
                fmt_num(i.importance, 1),
                i.terms,
                if i.terms == 1 { "" } else { "s" }
            ));
        }
    }

    out.push_str("\nResiduals\n");
    out.push_str(&format!(
        "  min {}  median {}  max {}\n",
        fmt_num(f.residual_min, d),
        fmt_num(f.residual_median, d),
        fmt_num(f.residual_max, d)
    ));

    if !f.predictions.is_empty() {
        out.push_str("\nPredictions\n");
        for p in &f.predictions {
            let inputs = f
                .features
                .iter()
                .zip(&p.inputs)
                .map(|(name, v)| format!("{name}={}", fmt_num(*v, d)))
                .collect::<Vec<_>>()
                .join(", ");
            out.push_str(&format!(
                "  {}  ->  {} = {}\n",
                inputs,
                f.target,
                fmt_num(p.predicted, d)
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

fn render_csv(f: &Fit, d: usize) -> String {
    let mut out = String::new();
    out.push_str("term,coefficient,degree\n");
    for t in &f.terms {
        out.push_str(&format!(
            "{},{},{}\n",
            csv_cell(&t.name),
            fmt_num(t.coefficient, d),
            t.degree
        ));
    }

    out.push_str("\nstatistic,value\n");
    out.push_str(&format!("observations,{}\n", f.observations));
    out.push_str(&format!("forward_terms,{}\n", f.forward_terms));
    out.push_str(&format!("kept_terms,{}\n", f.kept_terms));
    out.push_str(&format!("model_degree,{}\n", f.model_degree));
    out.push_str(&format!("rss,{}\n", fmt_num(f.rss, d)));
    out.push_str(&format!("r_squared,{}\n", fmt_num(f.r_squared, d)));
    out.push_str(&format!("gcv,{}\n", fmt_num(f.gcv, d)));
    out.push_str(&format!("grsq,{}\n", fmt_num(f.grsq, d)));
    out.push_str(&format!("rmse,{}\n", fmt_num(f.rmse, d)));
    out.push_str(&format!("mae,{}\n", fmt_num(f.mae, d)));

    if !f.importance.is_empty() {
        out.push_str("\nvariable,importance,terms\n");
        for i in &f.importance {
            out.push_str(&format!(
                "{},{},{}\n",
                csv_cell(&i.variable),
                fmt_num(i.importance, 1),
                i.terms
            ));
        }
    }

    out.push_str(&format!(
        "\nrow,{},fitted,residual\n",
        csv_cell(&f.target)
    ));
    for p in &f.points {
        out.push_str(&format!(
            "{},{},{},{}\n",
            p.row,
            fmt_num(p.actual, d),
            fmt_num(p.fitted, d),
            fmt_num(p.residual, d)
        ));
    }

    if !f.predictions.is_empty() {
        out.push_str(&format!(
            "\nrow,{},predicted_{}\n",
            f.features
                .iter()
                .map(|s| csv_cell(s))
                .collect::<Vec<_>>()
                .join(","),
            csv_cell(&f.target)
        ));
        for p in &f.predictions {
            out.push_str(&format!(
                "{},{},{}\n",
                p.row,
                p.inputs
                    .iter()
                    .map(|v| fmt_num(*v, d))
                    .collect::<Vec<_>>()
                    .join(","),
                fmt_num(p.predicted, d)
            ));
        }
    }
    out
}

/// Entry point shared by the chat block, the CLI and the page.
pub fn run(data: &str, o: &Options) -> Result<String, String> {
    let fmt = o.format.trim().to_ascii_lowercase();
    if !matches!(fmt.as_str(), "text" | "csv" | "json") {
        return Err(format!(
            "format must be 'text', 'csv' or 'json' (got '{}')",
            o.format
        ));
    }
    let f = fit(data, o)?;
    let d = o.decimals.min(12) as usize;
    Ok(match fmt.as_str() {
        "json" => serde_json::to_string_pretty(&f).map_err(|e| e.to_string())?,
        "csv" => render_csv(&f, d),
        _ => render_text(&f, d),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A clean V shape: y = |x - 5|, so MARS should recover a knot at x = 5 exactly.
    fn v_shape() -> String {
        (0..=10)
            .map(|x| format!("{x},{}", (x as f64 - 5.0).abs()))
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn recovers_a_hinge_knot_exactly() {
        let f = fit(&v_shape(), &Options::default()).unwrap();
        assert_eq!(f.kept_terms, 3, "intercept plus both hinge halves");
        assert!(
            f.r_squared > 0.999_999,
            "a V shape is exactly piecewise linear, got R² {}",
            f.r_squared
        );
        assert_eq!(f.knots.len(), 1);
        assert_eq!(f.knots[0].variable, "x1");
        assert_eq!(f.knots[0].knots, vec![5.0]);
        // An exact fit, up to the floating-point noise of the QR solve.
        assert!(f.rss < 1e-12, "rss {}", f.rss);
    }

    #[test]
    fn straight_line_needs_no_hinge() {
        // y = 2x + 1 is linear, so the pruning pass should keep a compact model.
        let data = (0..20)
            .map(|x| format!("{x},{}", 2 * x + 1))
            .collect::<Vec<_>>()
            .join("\n");
        let f = fit(&data, &Options::default()).unwrap();
        assert!(f.r_squared > 0.999_999);
        assert!(f.kept_terms <= 3, "kept {}", f.kept_terms);
        // Predicting past the data still follows the fitted slope.
        let o = Options {
            predict: "25".into(),
            ..Default::default()
        };
        let f2 = fit(&data, &o).unwrap();
        assert!((f2.predictions[0].predicted - 51.0).abs() < 1e-6);
    }

    #[test]
    fn picks_the_target_column_by_name() {
        let data = "temp,sales\n1,10\n2,12\n3,14\n4,16\n5,18";
        let f = fit(
            data,
            &Options {
                target: "sales".into(),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(f.target, "sales");
        assert_eq!(f.features, vec!["temp".to_string()]);
        assert!(f.equation.starts_with("sales = "));
    }

    #[test]
    fn uses_the_second_feature_when_it_carries_the_signal() {
        // y depends only on b: a hinge at b = 3.
        let mut rows = Vec::new();
        for a in 0..6 {
            for b in 0..6 {
                let y = (b as f64 - 3.0).max(0.0) * 4.0 + 1.0;
                rows.push(format!("{a},{b},{y}"));
            }
        }
        let data = format!("a,b,y\n{}", rows.join("\n"));
        let f = fit(&data, &Options::default()).unwrap();
        assert!(f.r_squared > 0.999, "R² {}", f.r_squared);
        assert_eq!(f.importance[0].variable, "b");
        assert!(f.knots.iter().any(|k| k.variable == "b"));
    }

    #[test]
    fn pruning_off_keeps_the_whole_forward_model() {
        let o = Options {
            prune: false,
            max_terms: 7,
            ..Default::default()
        };
        let f = fit(&v_shape(), &o).unwrap();
        assert_eq!(f.kept_terms, f.forward_terms);
        assert!(!f.pruned);
    }

    #[test]
    fn nprune_caps_the_kept_terms() {
        let o = Options {
            nprune: 2,
            max_terms: 9,
            ..Default::default()
        };
        let f = fit(&v_shape(), &o).unwrap();
        assert!(f.kept_terms <= 2, "kept {}", f.kept_terms);
    }

    #[test]
    fn degree_two_allows_an_interaction_term() {
        // y = x1 * x2 needs a product term to fit well.
        let mut rows = Vec::new();
        for a in 0..8 {
            for b in 0..8 {
                rows.push(format!("{a},{b},{}", a * b));
            }
        }
        let data = rows.join("\n");
        let one = fit(&data, &Options::default()).unwrap();
        let two = fit(
            &data,
            &Options {
                max_degree: 2,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(
            two.r_squared > one.r_squared,
            "degree 2 ({}) should beat degree 1 ({})",
            two.r_squared,
            one.r_squared
        );
        assert_eq!(two.max_degree, 2);
        assert!(two.terms.iter().any(|t| t.degree == 2));
    }

    #[test]
    fn json_and_csv_render() {
        let json = run(
            &v_shape(),
            &Options {
                format: "json".into(),
                ..Default::default()
            },
        )
        .unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["kept_terms"], 3);
        assert!(v["terms"].as_array().unwrap().len() == 3);

        let csv = run(
            &v_shape(),
            &Options {
                format: "csv".into(),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(csv.starts_with("term,coefficient,degree\n"));
        assert!(csv.contains("\nstatistic,value\n"));
        assert!(csv.contains("\nrow,y,fitted,residual\n"));
    }

    #[test]
    fn rejects_a_constant_target() {
        let err = fit("1,5\n2,5\n3,5\n4,5", &Options::default()).unwrap_err();
        assert!(err.contains("constant"), "{err}");
    }

    #[test]
    fn rejects_non_numeric_cells() {
        let err = fit("1,2\n2,oops\n3,4\n4,5", &Options::default()).unwrap_err();
        assert!(err.contains("row 2"), "{err}");
        assert!(err.contains("not a finite number"), "{err}");
    }

    #[test]
    fn rejects_ragged_rows() {
        let err = fit("1,2\n2,3,4\n3,4", &Options::default()).unwrap_err();
        assert!(err.contains("row 2"), "{err}");
    }

    #[test]
    fn rejects_an_unknown_target_column() {
        let err = fit(
            "a,b\n1,2\n2,3\n3,4",
            &Options {
                target: "nope".into(),
                ..Default::default()
            },
        )
        .unwrap_err();
        assert!(err.contains("not found"), "{err}");
    }

    #[test]
    fn rejects_a_bad_format() {
        let err = run(
            &v_shape(),
            &Options {
                format: "xml".into(),
                ..Default::default()
            },
        )
        .unwrap_err();
        assert!(err.contains("format must be"), "{err}");
    }

    #[test]
    fn rejects_a_predict_row_of_the_wrong_width() {
        let err = fit(
            &v_shape(),
            &Options {
                predict: "1,2".into(),
                ..Default::default()
            },
        )
        .unwrap_err();
        assert!(err.contains("feature"), "{err}");
    }

    #[test]
    fn output_is_deterministic() {
        let a = run(&v_shape(), &Options::default()).unwrap();
        let b = run(&v_shape(), &Options::default()).unwrap();
        assert_eq!(a, b);
    }
}
