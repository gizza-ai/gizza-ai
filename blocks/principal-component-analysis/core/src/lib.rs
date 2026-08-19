//! principal-component-analysis core — principal component analysis (PCA) on a
//! numeric data matrix (rows = observations, columns = variables).
//!
//! Columns are mean-centered and, by default, standardized to unit variance
//! (so the analysis runs on the correlation matrix rather than the covariance
//! matrix). The symmetric covariance/correlation matrix is then diagonalized
//! with the cyclic **Jacobi eigenvalue algorithm** — pure Rust, dependency-free
//! besides serde, deterministic (no RNG), so it produces identical numbers
//! under wasmi, wasm32 and native.
//!
//! Reported per component: the eigenvalue (variance along that axis), its
//! proportion of the total variance, the running cumulative proportion, and the
//! loadings (the eigenvector coefficients — the weight of each original
//! variable in that component). Each observation is also projected onto the
//! components to give its scores (the coordinates in component space).
//!
//! Component signs are eigenvector-arbitrary, so they are fixed by convention:
//! each component is flipped, if needed, so that its largest-magnitude loading
//! is positive (the same rule scikit-learn's `svd_flip` applies). That makes
//! the output stable across runs and platforms.

use serde::Serialize;

/// Maximum number of observations (rows) accepted.
pub const MAX_ROWS: usize = 20_000;
/// Maximum number of variables (columns) accepted.
pub const MAX_COLS: usize = 100;
/// How many score rows the formatted text output prints before truncating.
const SCORES_TEXT_LIMIT: usize = 20;
/// Bar width, in characters, that represents 100% of the total variance in the
/// text scree plot.
const SCREE_WIDTH: usize = 20;

/// One principal component.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Component {
    /// Component name — `PC1`, `PC2`, … in decreasing order of variance.
    pub name: String,
    /// Eigenvalue = the variance of the data along this component.
    pub eigenvalue: f64,
    /// Share of the total variance explained by this component (0–1).
    pub proportion: f64,
    /// Running total of `proportion` up to and including this component.
    pub cumulative: f64,
    /// Loadings — the eigenvector coefficient of each input variable, in column
    /// order. Unit length; sign-fixed so the largest-magnitude entry is positive.
    pub loadings: Vec<f64>,
}

/// Full PCA result.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Pca {
    /// Number of observations (rows) analysed.
    pub n: usize,
    /// Number of variables (columns) analysed.
    pub variables: usize,
    /// Variable names, in column order (from the header row, `labels`, or `v1`, `v2`, …).
    pub variable_names: Vec<String>,
    /// Whether columns were standardized to unit variance (correlation-matrix PCA).
    pub standardized: bool,
    /// Column means subtracted during centering, in column order.
    pub means: Vec<f64>,
    /// Column sample standard deviations (n−1 denominator), in column order.
    pub std_devs: Vec<f64>,
    /// Total variance across all variables = the sum of all eigenvalues.
    pub total_variance: f64,
    /// The retained components, most-variance first.
    pub components: Vec<Component>,
    /// Number of components needed to reach 90% of the total variance.
    pub components_for_90: usize,
    /// Number of components needed to reach 95% of the total variance.
    pub components_for_95: usize,
    /// Number of components needed to reach 99% of the total variance.
    pub components_for_99: usize,
    /// Components with an eigenvalue above 1 (Kaiser criterion; standardized data only).
    pub kaiser_components: usize,
    /// Projected coordinates: one row per observation, one value per retained component.
    pub scores: Vec<Vec<f64>>,
}

fn round6(v: f64) -> f64 {
    if !v.is_finite() {
        return v;
    }
    (v * 1e6).round() / 1e6
}

fn fmt_num(v: f64) -> String {
    if !v.is_finite() {
        return format!("{v}");
    }
    let s = format!("{:.6}", round6(v));
    let s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    if s.is_empty() || s == "-0" {
        "0".to_string()
    } else {
        s
    }
}

fn split_row(line: &str) -> Vec<&str> {
    line.split(|c: char| c.is_whitespace() || c == ',' || c == ';')
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .collect()
}

/// A parsed data matrix plus the header names found on the first row, if any.
struct Matrix {
    rows: Vec<Vec<f64>>,
    header: Option<Vec<String>>,
}

/// Parse a data matrix: one observation per non-blank line, columns split on
/// commas, tabs, semicolons or runs of whitespace. A first row whose tokens are
/// not all numbers is taken as a header of variable names. Every data row must
/// have the same number of columns.
fn parse_matrix(text: &str) -> Result<Matrix, String> {
    let mut header: Option<Vec<String>> = None;
    let mut rows: Vec<Vec<f64>> = Vec::new();
    let mut seen_data_line = false;

    for (li, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let toks = split_row(trimmed);
        if toks.is_empty() {
            continue;
        }
        if !seen_data_line {
            seen_data_line = true;
            let all_numeric = toks.iter().all(|t| t.parse::<f64>().is_ok());
            if !all_numeric {
                header = Some(toks.iter().map(|t| t.to_string()).collect());
                continue;
            }
        }
        let mut row = Vec::with_capacity(toks.len());
        for t in toks {
            let v: f64 = t
                .parse()
                .map_err(|_| format!("row {}: '{}' is not a number", li + 1, t))?;
            if !v.is_finite() {
                return Err(format!("row {}: '{}' is not a finite number", li + 1, t));
            }
            row.push(v);
        }
        rows.push(row);
        if rows.len() > MAX_ROWS {
            return Err(format!(
                "too many observations — this tool handles up to {MAX_ROWS} rows"
            ));
        }
    }

    if rows.is_empty() {
        return Err(
            "no data found — paste rows of numbers, one observation per line, columns separated by commas, tabs, semicolons or spaces"
                .into(),
        );
    }
    let ncol = rows[0].len();
    if ncol < 2 {
        return Err(format!(
            "need at least 2 variables (columns) to run PCA; row 1 has {ncol}"
        ));
    }
    if ncol > MAX_COLS {
        return Err(format!(
            "too many variables — this tool handles up to {MAX_COLS} columns, got {ncol}"
        ));
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
    if rows.len() < 2 {
        return Err(format!(
            "need at least 2 observations (rows) to compute variance; got {}",
            rows.len()
        ));
    }
    if let Some(h) = &header {
        if h.len() != ncol {
            return Err(format!(
                "the header row has {} names but the data has {ncol} columns",
                h.len()
            ));
        }
    }
    Ok(Matrix { rows, header })
}

/// Symmetric eigenvalue decomposition by the cyclic Jacobi method.
/// Returns (eigenvalues, eigenvectors-as-columns), unsorted.
fn jacobi_eigen(input: &[Vec<f64>]) -> (Vec<f64>, Vec<Vec<f64>>) {
    let n = input.len();
    let mut a = input.to_vec();
    let mut v = vec![vec![0.0f64; n]; n];
    for (i, row) in v.iter_mut().enumerate() {
        row[i] = 1.0;
    }
    if n == 1 {
        return (vec![a[0][0]], v);
    }
    for _sweep in 0..100 {
        let mut off = 0.0;
        for i in 0..n {
            for j in (i + 1)..n {
                off += a[i][j] * a[i][j];
            }
        }
        if off <= 1e-30 {
            break;
        }
        for p in 0..n {
            for q in (p + 1)..n {
                if a[p][q].abs() <= 1e-300 {
                    continue;
                }
                // Rotation angle that zeroes a[p][q] (Numerical Recipes form,
                // choosing the smaller rotation for numerical stability).
                let theta = (a[q][q] - a[p][p]) / (2.0 * a[p][q]);
                let t = theta.signum() / (theta.abs() + (theta * theta + 1.0).sqrt());
                let c = 1.0 / (t * t + 1.0).sqrt();
                let s = t * c;
                for k in 0..n {
                    let akp = a[k][p];
                    let akq = a[k][q];
                    a[k][p] = c * akp - s * akq;
                    a[k][q] = s * akp + c * akq;
                }
                for k in 0..n {
                    let apk = a[p][k];
                    let aqk = a[q][k];
                    a[p][k] = c * apk - s * aqk;
                    a[q][k] = s * apk + c * aqk;
                }
                for k in 0..n {
                    let vkp = v[k][p];
                    let vkq = v[k][q];
                    v[k][p] = c * vkp - s * vkq;
                    v[k][q] = s * vkp + c * vkq;
                }
            }
        }
    }
    let eigenvalues = (0..n).map(|i| a[i][i]).collect();
    (eigenvalues, v)
}

/// Resolve the variable names: explicit `labels` win, then a header row, then `v1, v2, …`.
fn resolve_names(labels: &str, header: &Option<Vec<String>>, ncol: usize) -> Result<Vec<String>, String> {
    let trimmed = labels.trim();
    if !trimmed.is_empty() {
        let names: Vec<String> = trimmed
            .split(|c: char| c == ',' || c == ';' || c == '\t')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        if names.len() != ncol {
            return Err(format!(
                "labels has {} names but the data has {ncol} columns",
                names.len()
            ));
        }
        return Ok(names);
    }
    if let Some(h) = header {
        return Ok(h.clone());
    }
    Ok((1..=ncol).map(|i| format!("v{i}")).collect())
}

/// Run PCA and return the structured result.
pub fn analyze(data: &str, labels: &str, components: usize, scale: bool) -> Result<Pca, String> {
    let Matrix { rows, header } = parse_matrix(data)?;
    let n = rows.len();
    let p = rows[0].len();
    let names = resolve_names(labels, &header, p)?;

    // Center (and optionally standardize) every column.
    let means: Vec<f64> = (0..p)
        .map(|j| rows.iter().map(|r| r[j]).sum::<f64>() / n as f64)
        .collect();
    let std_devs: Vec<f64> = (0..p)
        .map(|j| {
            let ss: f64 = rows.iter().map(|r| (r[j] - means[j]).powi(2)).sum();
            (ss / (n - 1) as f64).max(0.0).sqrt()
        })
        .collect();

    if scale {
        if let Some(j) = std_devs.iter().position(|s| *s <= 1e-12) {
            return Err(format!(
                "column '{}' is constant (zero variance), so it cannot be standardized — turn off 'standardize columns' to run PCA on the covariance matrix, or remove that column",
                names[j]
            ));
        }
    }
    let z: Vec<Vec<f64>> = rows
        .iter()
        .map(|r| {
            (0..p)
                .map(|j| {
                    let c = r[j] - means[j];
                    if scale {
                        c / std_devs[j]
                    } else {
                        c
                    }
                })
                .collect()
        })
        .collect();

    // Covariance (or correlation, when standardized) matrix.
    let denom = (n - 1) as f64;
    let mut cov = vec![vec![0.0f64; p]; p];
    for i in 0..p {
        for j in i..p {
            let s: f64 = z.iter().map(|r| r[i] * r[j]).sum::<f64>() / denom;
            cov[i][j] = s;
            cov[j][i] = s;
        }
    }

    let (raw_vals, raw_vecs) = jacobi_eigen(&cov);
    // Sort component indices by decreasing eigenvalue (ties broken by index for
    // a deterministic order).
    let mut order: Vec<usize> = (0..p).collect();
    order.sort_by(|&a, &b| {
        raw_vals[b]
            .partial_cmp(&raw_vals[a])
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.cmp(&b))
    });

    let eigenvalues: Vec<f64> = order.iter().map(|&i| raw_vals[i].max(0.0)).collect();
    let total: f64 = eigenvalues.iter().sum();
    if total <= 1e-12 {
        return Err(
            "every column is constant — there is no variance to decompose, so PCA is undefined"
                .into(),
        );
    }

    // Eigenvectors as rows (one per component), sign-fixed so the
    // largest-magnitude loading is positive.
    let mut vectors: Vec<Vec<f64>> = order
        .iter()
        .map(|&c| (0..p).map(|r| raw_vecs[r][c]).collect::<Vec<f64>>())
        .collect();
    for vec_ in vectors.iter_mut() {
        let mut best = 0usize;
        for i in 1..p {
            // Tolerance so exact ties (symmetric data) always resolve to the
            // earliest column instead of flipping on floating-point noise.
            if vec_[i].abs() > vec_[best].abs() + 1e-12 {
                best = i;
            }
        }
        if vec_[best] < 0.0 {
            for x in vec_.iter_mut() {
                *x = -*x;
            }
        }
    }

    // How many components to report.
    let keep = if components == 0 {
        p
    } else {
        components.min(p)
    };

    let mut cum = 0.0;
    let mut comps = Vec::with_capacity(keep);
    for (i, vec_) in vectors.iter().enumerate().take(keep) {
        let prop = eigenvalues[i] / total;
        cum += prop;
        comps.push(Component {
            name: format!("PC{}", i + 1),
            eigenvalue: round6(eigenvalues[i]),
            proportion: round6(prop),
            cumulative: round6(cum),
            loadings: vec_.iter().map(|v| round6(*v)).collect(),
        });
    }

    // Thresholds are computed over ALL components, not just the retained ones.
    let needed = |target: f64| -> usize {
        let mut c = 0.0;
        for (i, e) in eigenvalues.iter().enumerate() {
            c += e / total;
            if c >= target - 1e-12 {
                return i + 1;
            }
        }
        p
    };
    let kaiser = eigenvalues.iter().filter(|e| **e > 1.0 + 1e-9).count();

    let scores: Vec<Vec<f64>> = z
        .iter()
        .map(|row| {
            vectors
                .iter()
                .take(keep)
                .map(|v| round6((0..p).map(|j| row[j] * v[j]).sum::<f64>()))
                .collect()
        })
        .collect();

    Ok(Pca {
        n,
        variables: p,
        variable_names: names,
        standardized: scale,
        means: means.into_iter().map(round6).collect(),
        std_devs: std_devs.into_iter().map(round6).collect(),
        total_variance: round6(total),
        components: comps,
        components_for_90: needed(0.90),
        components_for_95: needed(0.95),
        components_for_99: needed(0.99),
        kaiser_components: kaiser,
        scores,
    })
}

/// Render a table: first column left-aligned, the rest right-aligned.
fn render_table(headers: &[String], rows: &[Vec<String>]) -> String {
    let ncol = headers.len();
    let mut widths: Vec<usize> = headers.iter().map(|h| h.chars().count()).collect();
    for r in rows {
        for (i, cell) in r.iter().enumerate().take(ncol) {
            widths[i] = widths[i].max(cell.chars().count());
        }
    }
    let mut out = String::new();
    let line = |cells: &[String]| -> String {
        let mut s = String::from("  ");
        for (i, cell) in cells.iter().enumerate().take(ncol) {
            let pad = widths[i].saturating_sub(cell.chars().count());
            if i == 0 {
                s.push_str(cell);
                s.push_str(&" ".repeat(pad));
            } else {
                s.push_str("   ");
                s.push_str(&" ".repeat(pad));
                s.push_str(cell);
            }
        }
        s.trim_end().to_string()
    };
    out.push_str(&line(headers));
    out.push('\n');
    for r in rows {
        out.push_str(&line(r));
        out.push('\n');
    }
    out
}

/// Text scree plot: one bar per component, its length proportional to that
/// component's share of the TOTAL variance (a full `SCREE_WIDTH` bar = 100%),
/// so the drop-off — the "elbow" you pick the component count at — is visible
/// without a chart.
fn scree_plot(components: &[Component]) -> String {
    let name_w = components
        .iter()
        .map(|c| c.name.chars().count())
        .max()
        .unwrap_or(3);
    let pcts: Vec<String> = components
        .iter()
        .map(|c| format!("{}%", fmt_num(round6(c.proportion * 100.0))))
        .collect();
    let pct_w = pcts.iter().map(|s| s.chars().count()).max().unwrap_or(0);
    let mut out = String::new();
    for (c, pct) in components.iter().zip(pcts.iter()) {
        let mut bars = (c.proportion * SCREE_WIDTH as f64).round() as usize;
        // A component with a small but non-zero share still gets a visible bar.
        if bars == 0 && c.proportion > 0.0 {
            bars = 1;
        }
        let bars = bars.min(SCREE_WIDTH);
        out.push_str(&format!(
            "  {:<name_w$}  {}{}  {:>pct_w$}\n",
            c.name,
            "\u{2588}".repeat(bars),
            " ".repeat(SCREE_WIDTH - bars),
            pct,
        ));
    }
    out
}

/// Human-readable PCA summary (`format = "text"`).
pub fn summary(data: &str, labels: &str, components: usize, scale: bool) -> Result<String, String> {
    let r = analyze(data, labels, components, scale)?;
    let k = r.components.len();
    let mut out = String::new();
    out.push_str(&format!(
        "PCA on {} observations × {} variables ({})\n\n",
        r.n,
        r.variables,
        if r.standardized {
            "correlation matrix — columns standardized to unit variance"
        } else {
            "covariance matrix — columns centered only"
        }
    ));

    out.push_str("Explained variance:\n");
    let rows: Vec<Vec<String>> = r
        .components
        .iter()
        .map(|c| {
            vec![
                c.name.clone(),
                fmt_num(c.eigenvalue),
                fmt_num(c.proportion),
                format!("{}%", fmt_num(round6(c.proportion * 100.0))),
                fmt_num(c.cumulative),
            ]
        })
        .collect();
    out.push_str(&render_table(
        &[
            "component".to_string(),
            "eigenvalue".to_string(),
            "proportion".to_string(),
            "percent".to_string(),
            "cumulative".to_string(),
        ],
        &rows,
    ));
    out.push_str(&format!("\nTotal variance: {}\n", fmt_num(r.total_variance)));
    out.push_str(&format!(
        "Components needed: {} for 90%, {} for 95%, {} for 99% of the variance.\n",
        r.components_for_90, r.components_for_95, r.components_for_99
    ));
    if r.standardized {
        out.push_str(&format!(
            "Kaiser criterion (eigenvalue > 1): {} component{}.\n",
            r.kaiser_components,
            if r.kaiser_components == 1 { "" } else { "s" }
        ));
    }

    out.push_str("\nScree plot (share of the total variance):\n");
    out.push_str(&scree_plot(&r.components));

    out.push_str("\nLoadings (weight of each variable in each component):\n");
    let mut head = vec!["variable".to_string()];
    head.extend(r.components.iter().map(|c| c.name.clone()));
    let rows: Vec<Vec<String>> = (0..r.variables)
        .map(|j| {
            let mut row = vec![r.variable_names[j].clone()];
            row.extend(r.components.iter().map(|c| fmt_num(c.loadings[j])));
            row
        })
        .collect();
    out.push_str(&render_table(&head, &rows));

    out.push_str(&format!(
        "\nScores (each observation projected onto the {} component{}):\n",
        k,
        if k == 1 { "" } else { "s" }
    ));
    let mut head = vec!["row".to_string()];
    head.extend(r.components.iter().map(|c| c.name.clone()));
    let shown = r.scores.len().min(SCORES_TEXT_LIMIT);
    let rows: Vec<Vec<String>> = r.scores[..shown]
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let mut row = vec![format!("{}", i + 1)];
            row.extend(s.iter().map(|v| fmt_num(*v)));
            row
        })
        .collect();
    out.push_str(&render_table(&head, &rows));
    if shown < r.scores.len() {
        out.push_str(&format!(
            "… {} more row{} — choose the CSV or JSON output for all {} scores.\n",
            r.scores.len() - shown,
            if r.scores.len() - shown == 1 { "" } else { "s" },
            r.scores.len()
        ));
    }
    Ok(out.trim_end().to_string())
}

/// CSV of the projected coordinates (`format = "csv"`): `row,PC1,PC2,…`.
pub fn scores_csv(data: &str, labels: &str, components: usize, scale: bool) -> Result<String, String> {
    let r = analyze(data, labels, components, scale)?;
    let mut out = String::from("row");
    for c in &r.components {
        out.push(',');
        out.push_str(&c.name);
    }
    out.push('\n');
    for (i, s) in r.scores.iter().enumerate() {
        out.push_str(&format!("{}", i + 1));
        for v in s {
            out.push(',');
            out.push_str(&fmt_num(*v));
        }
        out.push('\n');
    }
    Ok(out.trim_end().to_string())
}

/// Entry point shared by the chat block, the CLI and the web page.
pub fn run(
    data: &str,
    labels: &str,
    components: f64,
    scale: bool,
    format: &str,
) -> Result<String, String> {
    if !components.is_finite() || components < 0.0 || components.fract() != 0.0 {
        return Err("components must be a whole number ≥ 0 (0 = keep every component)".into());
    }
    if components > MAX_COLS as f64 {
        return Err(format!(
            "components must be at most {MAX_COLS} (the maximum number of variables)"
        ));
    }
    let k = components as usize;
    match format.trim().to_ascii_lowercase().as_str() {
        "" | "text" => summary(data, labels, k, scale),
        "csv" => scores_csv(data, labels, k, scale),
        "json" => {
            let r = analyze(data, labels, k, scale)?;
            serde_json::to_string_pretty(&r).map_err(|e| format!("could not serialize result: {e}"))
        }
        other => Err(format!(
            "unknown format '{other}' — use 'text', 'json' or 'csv'"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Perfectly correlated columns → PC1 carries all the variance.
    #[test]
    fn perfect_correlation_puts_everything_on_pc1() {
        let r = analyze("1,2\n2,4\n3,6\n4,8", "", 0, true).unwrap();
        assert_eq!(r.n, 4);
        assert_eq!(r.variables, 2);
        assert_eq!(r.components.len(), 2);
        assert_eq!(r.components[0].eigenvalue, 2.0);
        assert_eq!(r.components[0].proportion, 1.0);
        assert_eq!(r.components[1].eigenvalue, 0.0);
        assert_eq!(r.components_for_95, 1);
        // Standardized 2-variable PCA: loadings are ±1/√2.
        let expected = round6(1.0 / 2f64.sqrt());
        assert_eq!(r.components[0].loadings, vec![expected, expected]);
    }

    /// Anti-correlated columns still collapse to one component, with opposite signs.
    #[test]
    fn anti_correlated_columns_have_opposite_loadings() {
        let r = analyze("1,8\n2,6\n3,4\n4,2", "", 1, true).unwrap();
        assert_eq!(r.components.len(), 1);
        assert_eq!(r.components[0].proportion, 1.0);
        let l = &r.components[0].loadings;
        assert!(l[0] * l[1] < 0.0, "loadings must have opposite signs: {l:?}");
        // Sign convention: largest-magnitude loading is positive (here they tie,
        // so the first one wins).
        assert!(l[0] > 0.0);
    }

    /// Uncorrelated unit-variance columns split the variance evenly.
    #[test]
    fn independent_columns_split_variance() {
        let r = analyze("1,1\n2,-1\n3,-1\n4,1", "", 0, true).unwrap();
        assert_eq!(r.components[0].eigenvalue, 1.0);
        assert_eq!(r.components[1].eigenvalue, 1.0);
        assert_eq!(r.components[0].proportion, 0.5);
        assert_eq!(r.total_variance, 2.0);
        assert_eq!(r.kaiser_components, 0);
    }

    /// Covariance-matrix PCA (scale = false) is driven by the raw units.
    #[test]
    fn unscaled_pca_follows_the_larger_variance_column() {
        let r = analyze("0,0\n0,100\n0,200\n1,300", "", 0, false).unwrap();
        // Column 2 dominates, so PC1 is essentially that column.
        assert!(r.components[0].loadings[1].abs() > 0.99);
        assert!(r.components[0].proportion > 0.999);
        assert!(!r.standardized);
    }

    /// Scores are the centered/scaled rows projected on the components.
    #[test]
    fn scores_are_centered_projections() {
        let r = analyze("1,2\n2,4\n3,6\n4,8", "", 1, true).unwrap();
        assert_eq!(r.scores.len(), 4);
        assert_eq!(r.scores[0].len(), 1);
        // Symmetric data → scores sum to zero.
        let sum: f64 = r.scores.iter().map(|s| s[0]).sum();
        assert!(sum.abs() < 1e-9, "scores should be centered, got {sum}");
        assert!(r.scores[0][0] < 0.0 && r.scores[3][0] > 0.0);
    }

    /// The eigenvalues of a correlation matrix sum to the number of variables.
    #[test]
    fn correlation_eigenvalues_sum_to_variable_count() {
        let r = analyze("1,4,7\n2,9,1\n3,1,5\n4,6,2\n5,3,9", "", 0, true).unwrap();
        assert_eq!(r.total_variance, 3.0);
        let sum: f64 = r.components.iter().map(|c| c.eigenvalue).sum();
        assert!((sum - 3.0).abs() < 1e-6);
        assert_eq!(r.components.last().unwrap().cumulative, 1.0);
    }

    /// A header row of names is detected and used to label the loadings.
    #[test]
    fn header_row_names_the_variables() {
        let r = analyze("height,weight\n1,2\n2,4\n3,5", "", 0, true).unwrap();
        assert_eq!(r.variable_names, vec!["height", "weight"]);
        assert_eq!(r.n, 3);
    }

    /// Explicit labels win over a header row.
    #[test]
    fn labels_override_the_header_row() {
        let r = analyze("a,b\n1,2\n2,4\n3,5", "x,y", 0, true).unwrap();
        assert_eq!(r.variable_names, vec!["x", "y"]);
    }

    #[test]
    fn components_caps_the_reported_count() {
        let r = analyze("1,4,7\n2,9,1\n3,1,5\n4,6,2\n5,3,9", "", 2, true).unwrap();
        assert_eq!(r.components.len(), 2);
        assert_eq!(r.scores[0].len(), 2);
        // Thresholds still consider every component.
        assert!(r.components_for_99 >= 1);
    }

    #[test]
    fn text_output_has_the_expected_sections() {
        let s = summary("1,2\n2,4\n3,6\n4,8", "x,y", 0, true).unwrap();
        assert!(s.starts_with("PCA on 4 observations × 2 variables (correlation matrix"));
        assert!(s.contains("Explained variance:"));
        assert!(s.contains("PC1"));
        assert!(s.contains("Loadings (weight of each variable in each component):"));
        assert!(s.contains("Scores"));
        assert!(s.contains("Components needed: 1 for 90%, 1 for 95%, 1 for 99%"));
    }

    /// The scree bars are scaled against the TOTAL variance: a component holding
    /// everything fills the bar, one holding nothing gets none.
    #[test]
    fn scree_plot_bars_track_the_variance_share() {
        let s = summary("1,2\n2,4\n3,6\n4,8", "x,y", 0, true).unwrap();
        assert!(s.contains("Scree plot (share of the total variance):"));
        assert!(s.contains(&format!("PC1  {}  100%", "\u{2588}".repeat(20))));
        // The percent column is right-aligned to the widest entry ("100%").
        assert!(s.contains(&format!("PC2  {}    0%", " ".repeat(20))));
        // An even split gives each component half a bar.
        let s = summary("1,1\n2,-1\n3,-1\n4,1", "", 0, true).unwrap();
        assert!(s.contains(&format!("PC1  {}{}  50%", "\u{2588}".repeat(10), " ".repeat(10))));
    }

    #[test]
    fn csv_output_lists_every_score_row() {
        let csv = scores_csv("1,2\n2,4\n3,6\n4,8", "", 1, true).unwrap();
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines[0], "row,PC1");
        assert_eq!(lines.len(), 5);
        assert!(lines[1].starts_with("1,"));
    }

    #[test]
    fn json_output_round_trips() {
        let j = run("1,2\n2,4\n3,6\n4,8", "", 0.0, true, "json").unwrap();
        assert!(j.contains("\"total_variance\": 2"));
        assert!(j.contains("\"standardized\": true"));
        assert!(j.contains("\"scores\""));
        assert!(j.contains("\"loadings\""));
    }

    #[test]
    fn long_input_truncates_the_text_score_table() {
        let data: String = (1..=25).map(|i| format!("{i},{}\n", i * 2 + i % 3)).collect();
        let s = summary(&data, "", 0, true).unwrap();
        assert!(s.contains("… 5 more rows"));
        let csv = scores_csv(&data, "", 0, true).unwrap();
        assert_eq!(csv.lines().count(), 26);
    }

    // ---- errors ----

    #[test]
    fn rejects_a_constant_column_when_standardizing() {
        let e = analyze("1,5\n2,5\n3,5", "", 0, true).unwrap_err();
        assert!(e.contains("constant"), "{e}");
        // …but the covariance-matrix path handles it fine.
        assert!(analyze("1,5\n2,5\n3,5", "", 0, false).is_ok());
    }

    #[test]
    fn rejects_ragged_rows() {
        let e = analyze("1,2\n3,4,5", "", 0, true).unwrap_err();
        assert!(e.contains("same number of columns"), "{e}");
    }

    #[test]
    fn rejects_a_single_column() {
        let e = analyze("1\n2\n3", "", 0, true).unwrap_err();
        assert!(e.contains("at least 2 variables"), "{e}");
    }

    #[test]
    fn rejects_a_single_observation() {
        let e = analyze("1,2", "", 0, true).unwrap_err();
        assert!(e.contains("at least 2 observations"), "{e}");
    }

    #[test]
    fn rejects_non_numeric_data() {
        let e = analyze("1,2\n3,abc", "", 0, true).unwrap_err();
        assert!(e.contains("is not a number"), "{e}");
    }

    #[test]
    fn rejects_empty_input() {
        let e = analyze("   \n\n", "", 0, true).unwrap_err();
        assert!(e.contains("no data found"), "{e}");
    }

    #[test]
    fn rejects_a_mismatched_label_count() {
        let e = analyze("1,2\n2,4\n3,5", "only-one", 0, true).unwrap_err();
        assert!(e.contains("labels has 1 names"), "{e}");
    }

    #[test]
    fn rejects_an_unknown_format() {
        let e = run("1,2\n2,4", "", 0.0, true, "xml").unwrap_err();
        assert!(e.contains("unknown format"), "{e}");
    }

    #[test]
    fn rejects_a_fractional_component_count() {
        let e = run("1,2\n2,4", "", 1.5, true, "text").unwrap_err();
        assert!(e.contains("whole number"), "{e}");
    }

    #[test]
    fn rejects_too_many_columns() {
        let row: String = (0..MAX_COLS + 1)
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let e = analyze(&format!("{row}\n{row}"), "", 0, true).unwrap_err();
        assert!(e.contains("too many variables"), "{e}");
    }
}
