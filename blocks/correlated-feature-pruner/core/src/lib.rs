//! correlated-feature-pruner core — pure compute, shared by the chat skill block
//! and the web page. No wafer/wasm-bindgen deps.
//!
//! Parses rows of numbers (each column = a feature/variable, each row = an
//! observation), computes pairwise correlation (Pearson, Spearman, or Kendall),
//! and greedily drops one column from every pair whose ABSOLUTE correlation
//! exceeds a threshold — reducing multicollinearity before modeling. The kept set
//! is built left-to-right: a column is dropped when it correlates too strongly
//! with a column already kept (the deterministic "keep the first, drop the later"
//! rule the common pandas/feature-engine recipes use), and each drop reports the
//! keeper it was redundant with plus the correlation value.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Method {
    Pearson,
    Spearman,
    Kendall,
}

impl Method {
    pub fn parse(s: &str) -> Result<Method, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "pearson" => Ok(Method::Pearson),
            "spearman" => Ok(Method::Spearman),
            "kendall" => Ok(Method::Kendall),
            other => Err(format!(
                "method must be 'pearson', 'spearman', or 'kendall' (got '{other}')"
            )),
        }
    }
    fn label(self) -> &'static str {
        match self {
            Method::Pearson => "pearson",
            Method::Spearman => "spearman",
            Method::Kendall => "kendall",
        }
    }
}

/// One dropped column: its index, the kept column it was redundant with, and the
/// signed correlation between them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Dropped {
    pub col: usize,
    pub keeper: usize,
    pub corr: f64,
}

/// The pruning decision: which column indices survive, and which were dropped.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Pruned {
    pub kept: Vec<usize>,
    pub dropped: Vec<Dropped>,
}

/// Parsed input: column display names + column-major numeric data (`cols[var][obs]`).
struct Table {
    names: Vec<String>,
    cols: Vec<Vec<f64>>,
}

fn split_tokens(line: &str) -> Vec<&str> {
    line.split([',', '\t'])
        .flat_map(|t| t.split(' '))
        .map(|t| t.trim())
        .filter(|t| !t.is_empty())
        .collect()
}

/// Parse `data` into named columns. When `header` is true the first non-empty
/// line supplies column names (and is not treated as data). `labels_csv`, when
/// non-empty and matching the column count, overrides the names; otherwise
/// unnamed columns default to `v1..vN`.
fn parse_table(data: &str, header: bool, labels_csv: &str) -> Result<Table, String> {
    let mut lines = data.lines().enumerate().filter(|(_, l)| !l.trim().is_empty());

    let mut header_names: Option<Vec<String>> = None;
    if header {
        match lines.next() {
            Some((_, l)) => {
                header_names = Some(split_tokens(l).into_iter().map(|s| s.to_string()).collect());
            }
            None => return Err("input is empty".into()),
        }
    }

    let mut rows: Vec<Vec<f64>> = Vec::new();
    for (li, line) in lines {
        let mut row = Vec::new();
        for tok in split_tokens(line) {
            let v: f64 = tok
                .parse()
                .map_err(|_| format!("row {}: '{tok}' is not a number", li + 1))?;
            if !v.is_finite() {
                return Err(format!("row {}: non-finite value '{tok}'", li + 1));
            }
            row.push(v);
        }
        rows.push(row);
    }

    if rows.len() < 2 {
        return Err("need at least 2 data rows (observations) to correlate".into());
    }
    let ncol = rows[0].len();
    if ncol < 2 {
        return Err("need at least 2 columns (features) to compare".into());
    }
    if rows.iter().any(|r| r.len() != ncol) {
        return Err("all rows must have the same number of columns".into());
    }
    if let Some(ref h) = header_names {
        if h.len() != ncol {
            return Err(format!(
                "header has {} names but the data has {ncol} columns",
                h.len()
            ));
        }
    }

    let mut cols = vec![Vec::with_capacity(rows.len()); ncol];
    for r in &rows {
        for (j, &v) in r.iter().enumerate() {
            cols[j].push(v);
        }
    }

    // Name precedence: explicit labels > header row > v1..vN.
    let given: Vec<String> = labels_csv
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    let names = if given.len() == ncol {
        given
    } else if let Some(h) = header_names.filter(|h| h.len() == ncol) {
        h
    } else {
        (1..=ncol).map(|i| format!("v{i}")).collect()
    };

    Ok(Table { names, cols })
}

fn pearson(x: &[f64], y: &[f64]) -> f64 {
    let n = x.len() as f64;
    let mx = x.iter().sum::<f64>() / n;
    let my = y.iter().sum::<f64>() / n;
    let (mut cov, mut vx, mut vy) = (0.0, 0.0, 0.0);
    for i in 0..x.len() {
        let dx = x[i] - mx;
        let dy = y[i] - my;
        cov += dx * dy;
        vx += dx * dx;
        vy += dy * dy;
    }
    if vx == 0.0 || vy == 0.0 {
        // A constant column has undefined correlation; treat it as 0 (never
        // redundant), so it is neither dropped nor forces a drop.
        return 0.0;
    }
    (cov / (vx.sqrt() * vy.sqrt())).clamp(-1.0, 1.0)
}

/// Fractional ranks (average ranks for ties) — the Spearman transform.
fn ranks(x: &[f64]) -> Vec<f64> {
    let mut idx: Vec<usize> = (0..x.len()).collect();
    idx.sort_by(|&a, &b| x[a].partial_cmp(&x[b]).unwrap());
    let mut r = vec![0.0; x.len()];
    let mut i = 0;
    while i < idx.len() {
        let mut j = i;
        while j + 1 < idx.len() && x[idx[j + 1]] == x[idx[i]] {
            j += 1;
        }
        let avg = ((i + j) as f64) / 2.0 + 1.0; // 1-based average rank
        for k in i..=j {
            r[idx[k]] = avg;
        }
        i = j + 1;
    }
    r
}

/// Kendall's tau-b (tie-corrected). O(n²) — fine for the browser-sized datasets
/// this tool handles.
fn kendall_tau_b(x: &[f64], y: &[f64]) -> f64 {
    let n = x.len();
    let (mut conc, mut disc, mut n1, mut n2) = (0i64, 0i64, 0i64, 0i64);
    for i in 0..n {
        for j in (i + 1)..n {
            let dx = x[j] - x[i];
            let dy = y[j] - y[i];
            if dx == 0.0 {
                n1 += 1;
            }
            if dy == 0.0 {
                n2 += 1;
            }
            if dx != 0.0 && dy != 0.0 {
                if (dx > 0.0) == (dy > 0.0) {
                    conc += 1;
                } else {
                    disc += 1;
                }
            }
        }
    }
    let n0 = (n as i64) * (n as i64 - 1) / 2;
    let denom = (((n0 - n1) as f64) * ((n0 - n2) as f64)).sqrt();
    if denom == 0.0 {
        return 0.0;
    }
    ((conc - disc) as f64 / denom).clamp(-1.0, 1.0)
}

/// Prepare each column for correlation (rank-transform for Spearman; identity for
/// Pearson and Kendall, which operate on raw values).
fn prepared(cols: &[Vec<f64>], method: Method) -> Vec<Vec<f64>> {
    match method {
        Method::Spearman => cols.iter().map(|c| ranks(c)).collect(),
        _ => cols.to_vec(),
    }
}

/// Correlation between two prepared columns under the chosen method.
fn pair_corr(a: &[f64], b: &[f64], method: Method) -> f64 {
    match method {
        Method::Kendall => kendall_tau_b(a, b),
        // Pearson on raw values (Pearson) or on pre-computed ranks (Spearman).
        _ => pearson(a, b),
    }
}

/// Greedy left-to-right pruning: keep a column unless it correlates
/// (|corr| > threshold) with a column already kept, in which case drop it and
/// record the most-correlated keeper.
pub fn prune(cols: &[Vec<f64>], method: Method, threshold: f64) -> Pruned {
    let prep = prepared(cols, method);
    let mut out = Pruned::default();
    for j in 0..prep.len() {
        let mut best: Option<(usize, f64)> = None;
        for &k in &out.kept {
            let c = pair_corr(&prep[j], &prep[k], method);
            if c.abs() > threshold && best.map_or(true, |(_, b)| c.abs() > b.abs()) {
                best = Some((k, c));
            }
        }
        match best {
            Some((keeper, corr)) => out.dropped.push(Dropped { col: j, keeper, corr }),
            None => out.kept.push(j),
        }
    }
    out
}

fn fmt_num(v: f64) -> String {
    // Trim a trailing ".0" so integer-valued cells stay clean in the emitted CSV.
    if v == v.trunc() && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

/// Full pruning report: a summary line, the kept/dropped breakdown, and the
/// pruned CSV (kept columns only, with a header row of their names).
pub fn prune_report(
    data: &str,
    threshold: f64,
    method_str: &str,
    labels_csv: &str,
    header: bool,
) -> Result<String, String> {
    let method = Method::parse(method_str)?;
    if !threshold.is_finite() || !(0.0..=1.0).contains(&threshold) {
        return Err(format!("threshold must be between 0 and 1 (got {threshold})"));
    }
    let Table { names, cols } = parse_table(data, header, labels_csv)?;
    let n = cols.len();
    let res = prune(&cols, method, threshold);

    let mut s = String::new();
    s.push_str(&format!(
        "Kept {} of {n} columns (threshold |r| > {:.2}, {}).\n\n",
        res.kept.len(),
        threshold,
        method.label()
    ));

    let kept_names: Vec<&str> = res.kept.iter().map(|&i| names[i].as_str()).collect();
    s.push_str(&format!("Kept ({}): {}\n", res.kept.len(), kept_names.join(", ")));

    if res.dropped.is_empty() {
        s.push_str("Dropped (0): none — no pair exceeded the threshold.\n");
    } else {
        s.push_str(&format!("Dropped ({}):\n", res.dropped.len()));
        for d in &res.dropped {
            s.push_str(&format!(
                "  {} (|r|={:.2} with {})\n",
                names[d.col],
                d.corr.abs(),
                names[d.keeper]
            ));
        }
    }

    // Pruned CSV: header row of kept names + each observation's kept-column values.
    s.push_str("\nPruned data:\n");
    s.push_str(&kept_names.join(","));
    s.push('\n');
    let nobs = cols.first().map_or(0, |c| c.len());
    for obs in 0..nobs {
        let row: Vec<String> = res.kept.iter().map(|&i| fmt_num(cols[i][obs])).collect();
        s.push_str(&row.join(","));
        s.push('\n');
    }

    // Drop the trailing newline for a clean, exact-comparable output.
    while s.ends_with('\n') {
        s.pop();
    }
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drops_perfectly_correlated_column() {
        // Columns: a, 2*a (perfectly correlated), b (independent-ish).
        let data = "1,2,5\n2,4,3\n3,6,9\n4,8,1";
        let out = prune_report(data, 0.9, "pearson", "a,a2,b", false).unwrap();
        assert!(out.starts_with("Kept 2 of 3 columns (threshold |r| > 0.90, pearson)."));
        assert!(out.contains("Kept (2): a, b"));
        assert!(out.contains("a2 (|r|=1.00 with a)"));
        // Pruned CSV keeps only a and b.
        assert!(out.contains("\nPruned data:\na,b\n1,5\n2,3\n3,9\n4,1"));
    }

    #[test]
    fn keeps_all_when_below_threshold() {
        let data = "1,5\n2,3\n3,9\n4,1";
        let out = prune_report(data, 0.99, "pearson", "", false).unwrap();
        assert!(out.starts_with("Kept 2 of 2 columns"));
        assert!(out.contains("Dropped (0): none"));
    }

    #[test]
    fn absolute_value_catches_negative_correlation() {
        // v2 = -3*v1 → r = -1.0; |r| must trip the threshold.
        let data = "1,-3\n2,-6\n3,-9\n4,-12";
        let out = prune_report(data, 0.9, "pearson", "x,negx", false).unwrap();
        assert!(out.contains("Kept (1): x"));
        assert!(out.contains("negx (|r|=1.00 with x)"));
    }

    #[test]
    fn threshold_boundary_is_exclusive() {
        // v2 == 2*v1 → |r| == 1.0 exactly.
        let cols = [vec![1.0, 2.0, 3.0], vec![2.0, 4.0, 6.0]];
        // At threshold 1.0, 1.0 > 1.0 is false → keep both.
        let at = prune(&cols, Method::Pearson, 1.0);
        assert_eq!(at.kept.len(), 2);
        assert!(at.dropped.is_empty());
        // Just under 1.0 → drop one.
        let over = prune(&cols, Method::Pearson, 0.99);
        assert_eq!(over.kept, vec![0]);
        assert_eq!(over.dropped.len(), 1);
    }

    #[test]
    fn spearman_catches_monotonic_nonlinear() {
        // y = x^3 is monotonic but nonlinear: Spearman |r| == 1, Pearson < 1.
        let data = "1,1\n2,8\n3,27\n4,64";
        let sp = prune_report(data, 0.99, "spearman", "x,x3", false).unwrap();
        assert!(sp.contains("x3 (|r|=1.00 with x)"));
        // Pearson at the same threshold keeps both (curvature drops |r| below 0.99).
        let pe = prune_report(data, 0.99, "pearson", "x,x3", false).unwrap();
        assert!(pe.contains("Dropped (0): none"));
    }

    #[test]
    fn kendall_is_available() {
        // Perfectly monotonic → tau-b == 1.
        let data = "1,10\n2,20\n3,30\n4,40";
        let out = prune_report(data, 0.9, "kendall", "x,y", false).unwrap();
        assert!(out.contains("(threshold |r| > 0.90, kendall)"));
        assert!(out.contains("y (|r|=1.00 with x)"));
    }

    #[test]
    fn header_row_names_columns() {
        let data = "age,income\n20,100\n30,200\n40,300";
        let out = prune_report(data, 0.9, "pearson", "", true).unwrap();
        // age and income are perfectly correlated here → one dropped.
        assert!(out.contains("Kept (1): age"));
        assert!(out.contains("income (|r|=1.00 with age)"));
    }

    #[test]
    fn err_non_numeric() {
        let e = prune_report("1,2\nx,4\n3,6", 0.9, "pearson", "", false).unwrap_err();
        assert!(e.contains("is not a number"), "got: {e}");
    }

    #[test]
    fn err_bad_method() {
        let e = prune_report("1,2\n3,4", 0.9, "mutual-info", "", false).unwrap_err();
        assert!(e.contains("pearson"), "got: {e}");
    }

    #[test]
    fn err_bad_threshold() {
        let e = prune_report("1,2\n3,4", 1.5, "pearson", "", false).unwrap_err();
        assert!(e.contains("between 0 and 1"), "got: {e}");
    }

    #[test]
    fn err_too_few_columns() {
        let e = prune_report("1\n2\n3", 0.9, "pearson", "", false).unwrap_err();
        assert!(e.contains("at least 2 columns"), "got: {e}");
    }
}
