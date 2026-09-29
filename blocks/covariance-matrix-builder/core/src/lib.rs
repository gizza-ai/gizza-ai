//! covariance-matrix-builder core — builds the covariance matrix of a pasted
//! multivariate dataset (rows = observations, columns = variables), plus the
//! standardized variants of the same data.
//!
//! Four related outputs share one parse + one centering pass:
//!
//! * `covariance`   — the symmetric k×k matrix `Σ`, where
//!   `Σ[i][j] = Σ_r w_r (x_ri − m_i)(x_rj − m_j) / denom`. `denom` is `Σw − 1`
//!   for the **sample** convention (Bessel's correction — what `numpy.cov`,
//!   `COVARIANCE.S` and R's `cov()` use) or `Σw` for the **population**
//!   convention (`COVARIANCE.P`). Unweighted data has `w_r = 1`, so `Σw = n`.
//! * `correlation`  — the same matrix standardized by the column standard
//!   deviations, `Σ[i][j] / (s_i s_j)`; i.e. Pearson correlation, which is why
//!   the sample/population choice cancels out of it.
//! * `centered`     — the data matrix itself with each column mean removed.
//! * `standardized` — the data matrix as z-scores, `(x − m) / s`. Its covariance
//!   matrix IS the correlation matrix, which is the whole point of the variant.
//!
//! Pure Rust, dependency-free besides serde, no RNG and no floating-point
//! reductions that depend on iteration order, so the numbers are identical under
//! wasmi, wasm32 and native.

use serde::Serialize;

/// Maximum number of observations (rows) accepted.
pub const MAX_ROWS: usize = 20_000;
/// Maximum number of variables (columns) accepted.
pub const MAX_COLS: usize = 100;
/// Maximum number of decimal places that can be requested.
pub const MAX_DECIMALS: usize = 12;

/// Which matrix the caller asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatrixKind {
    /// The covariance matrix itself.
    Covariance,
    /// The covariance matrix standardized to correlations.
    Correlation,
    /// The input data with each column's mean subtracted.
    Centered,
    /// The input data as z-scores.
    Standardized,
}

impl MatrixKind {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "covariance" | "cov" => Ok(MatrixKind::Covariance),
            "correlation" | "corr" => Ok(MatrixKind::Correlation),
            "centered" | "centred" => Ok(MatrixKind::Centered),
            "standardized" | "standardised" | "zscore" => Ok(MatrixKind::Standardized),
            other => Err(format!(
                "unknown matrix '{other}' — use covariance, correlation, centered or standardized"
            )),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            MatrixKind::Covariance => "covariance",
            MatrixKind::Correlation => "correlation",
            MatrixKind::Centered => "centered",
            MatrixKind::Standardized => "standardized",
        }
    }

    /// True when the output is the k×k matrix rather than the n×k data matrix.
    fn is_matrix(self) -> bool {
        matches!(self, MatrixKind::Covariance | MatrixKind::Correlation)
    }

    /// True when column standard deviations are needed (so a constant column is
    /// a hard error rather than a harmless zero row).
    fn needs_std_dev(self) -> bool {
        matches!(self, MatrixKind::Correlation | MatrixKind::Standardized)
    }
}

/// Sample (n−1) or population (n) denominator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Denominator {
    /// `Σw − 1` — Bessel-corrected, the default everywhere.
    Sample,
    /// `Σw` — use when the rows are the entire population.
    Population,
}

impl Denominator {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "sample" | "s" | "n-1" => Ok(Denominator::Sample),
            "population" | "p" | "n" => Ok(Denominator::Population),
            other => Err(format!(
                "unknown denominator '{other}' — use sample (n−1) or population (n)"
            )),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Denominator::Sample => "sample",
            Denominator::Population => "population",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Delim {
    Auto,
    Comma,
    Tab,
    Semicolon,
    Pipe,
    Space,
}

impl Delim {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "auto" => Ok(Delim::Auto),
            "comma" | "," => Ok(Delim::Comma),
            "tab" | "\t" => Ok(Delim::Tab),
            "semicolon" | ";" => Ok(Delim::Semicolon),
            "pipe" | "|" => Ok(Delim::Pipe),
            "space" | " " => Ok(Delim::Space),
            other => Err(format!(
                "unknown delimiter '{other}' — use auto, comma, tab, semicolon, space or pipe"
            )),
        }
    }

    fn split(self, line: &str) -> Vec<String> {
        let by = |c: char| -> Vec<String> {
            line.split(c).map(|t| t.trim().to_string()).collect()
        };
        match self {
            Delim::Auto => line
                .split(|c: char| c.is_whitespace() || c == ',' || c == ';' || c == '|')
                .map(str::trim)
                .filter(|t| !t.is_empty())
                .map(|t| t.to_string())
                .collect(),
            Delim::Comma => by(','),
            Delim::Tab => by('\t'),
            Delim::Semicolon => by(';'),
            Delim::Pipe => by('|'),
            Delim::Space => line
                .split_whitespace()
                .map(|t| t.to_string())
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Header {
    Auto,
    Yes,
    No,
}

impl Header {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "auto" => Ok(Header::Auto),
            "yes" | "true" | "1" => Ok(Header::Yes),
            "no" | "false" | "0" => Ok(Header::No),
            other => Err(format!(
                "unknown header '{other}' — use auto, yes or no"
            )),
        }
    }
}

/// The computed result, ready to serialize.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CovarianceResult {
    /// Which variant was produced: covariance, correlation, centered or standardized.
    pub kind: String,
    /// Denominator convention used for the covariance/variance figures.
    pub denominator: String,
    /// Number of observations (data rows) used.
    pub n: usize,
    /// Number of variables (columns).
    pub variables: usize,
    /// Variable names, in column order.
    pub variable_names: Vec<String>,
    /// Whether per-row weights were supplied.
    pub weighted: bool,
    /// Sum of the weights (= `n` when unweighted).
    pub weight_sum: f64,
    /// Column means, in column order.
    pub means: Vec<f64>,
    /// Column variances (the covariance-matrix diagonal), in column order.
    pub variances: Vec<f64>,
    /// Column standard deviations = √variance, in column order.
    pub std_devs: Vec<f64>,
    /// Sum of the variances — the trace of the covariance matrix.
    pub total_variance: f64,
    /// The requested matrix: k×k for covariance/correlation, n×k for the
    /// centered/standardized data.
    pub matrix: Vec<Vec<f64>>,
}

fn round_to(v: f64, decimals: usize) -> f64 {
    if !v.is_finite() {
        return v;
    }
    let f = 10f64.powi(decimals as i32);
    let r = (v * f).round() / f;
    if r == 0.0 {
        0.0
    } else {
        r
    }
}

/// Fixed-point rendering with `decimals` places, normalising `-0` to `0`.
fn fmt_fixed(v: f64, decimals: usize) -> String {
    if !v.is_finite() {
        return format!("{v}");
    }
    let s = format!("{:.*}", decimals, round_to(v, decimals));
    if s.starts_with('-') && s[1..].chars().all(|c| c == '0' || c == '.') {
        s[1..].to_string()
    } else {
        s
    }
}

/// A parsed data matrix plus the header names found on the first row, if any.
struct Parsed {
    rows: Vec<Vec<f64>>,
    header: Option<Vec<String>>,
}

fn parse_matrix(text: &str, delim: Delim, header_mode: Header) -> Result<Parsed, String> {
    let mut header: Option<Vec<String>> = None;
    let mut rows: Vec<Vec<f64>> = Vec::new();
    let mut first_line = true;

    for (li, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let toks = delim.split(line);
        if toks.is_empty() || toks.iter().all(|t| t.is_empty()) {
            continue;
        }
        if first_line {
            first_line = false;
            let all_numeric = toks.iter().all(|t| t.parse::<f64>().is_ok());
            let is_header = match header_mode {
                Header::Auto => !all_numeric,
                Header::Yes => true,
                Header::No => false,
            };
            if is_header {
                header = Some(
                    toks.iter()
                        .enumerate()
                        .map(|(i, t)| {
                            if t.is_empty() {
                                format!("v{}", i + 1)
                            } else {
                                t.clone()
                            }
                        })
                        .collect(),
                );
                continue;
            }
        }
        let mut row = Vec::with_capacity(toks.len());
        for t in &toks {
            if t.is_empty() {
                return Err(format!(
                    "row {}: empty cell — every cell must hold a number (rows with missing values are not supported)",
                    li + 1
                ));
            }
            let v: f64 = t
                .parse()
                .map_err(|_| format!("row {}: '{}' is not a number", li + 1, t))?;
            if !v.is_finite() {
                return Err(format!("row {}: '{}' is not a finite number", li + 1, t));
            }
            row.push(v);
        }
        if let Some(first) = rows.first() {
            if row.len() != first.len() {
                return Err(format!(
                    "row {}: has {} columns but the first data row has {} — every row must have the same number of columns",
                    li + 1,
                    row.len(),
                    first.len()
                ));
            }
        }
        rows.push(row);
        if rows.len() > MAX_ROWS {
            return Err(format!("too many rows — at most {MAX_ROWS} observations"));
        }
    }
    Ok(Parsed { rows, header })
}

fn parse_weights(text: &str, n: usize, denom: Denominator) -> Result<Option<Vec<f64>>, String> {
    if text.trim().is_empty() {
        return Ok(None);
    }
    let mut w = Vec::new();
    for t in text
        .split(|c: char| c.is_whitespace() || c == ',' || c == ';')
        .map(str::trim)
        .filter(|t| !t.is_empty())
    {
        let v: f64 = t
            .parse()
            .map_err(|_| format!("weights: '{t}' is not a number"))?;
        if !v.is_finite() || v < 0.0 {
            return Err(format!(
                "weights: '{t}' is not a finite weight ≥ 0"
            ));
        }
        w.push(v);
    }
    if w.len() != n {
        return Err(format!(
            "weights: got {} values but there are {} data rows — supply one weight per row",
            w.len(),
            n
        ));
    }
    let sum: f64 = w.iter().sum();
    if sum <= 0.0 {
        return Err("weights: the weights must not all be zero".to_string());
    }
    if denom == Denominator::Sample && sum <= 1.0 {
        return Err(format!(
            "weights: the sample denominator is Σw − 1, but Σw is {sum} — increase the weights or use denominator=population"
        ));
    }
    Ok(Some(w))
}

fn resolve_names(
    header: Option<Vec<String>>,
    labels: &str,
    k: usize,
) -> Result<Vec<String>, String> {
    if !labels.trim().is_empty() {
        let names: Vec<String> = labels
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        if names.len() != k {
            return Err(format!(
                "labels: got {} names but the data has {} columns",
                names.len(),
                k
            ));
        }
        return Ok(names);
    }
    if let Some(h) = header {
        if h.len() != k {
            return Err(format!(
                "header row has {} names but the data has {} columns",
                h.len(),
                k
            ));
        }
        return Ok(h);
    }
    Ok((1..=k).map(|i| format!("v{i}")).collect())
}

/// Compute every variant from one parse. `decimals` only rounds the reported
/// numbers; the arithmetic itself is full `f64` precision.
#[allow(clippy::too_many_arguments)]
pub fn compute(
    data: &str,
    labels: &str,
    delimiter: &str,
    header: &str,
    matrix: &str,
    denominator: &str,
    weights: &str,
    decimals: f64,
) -> Result<CovarianceResult, String> {
    if decimals.fract() != 0.0 {
        return Err("decimals must be a whole number".to_string());
    }
    if decimals < 0.0 || decimals > MAX_DECIMALS as f64 {
        return Err(format!("decimals must be between 0 and {MAX_DECIMALS}"));
    }
    let decimals = decimals as usize;

    let kind = MatrixKind::parse(matrix)?;
    let denom_kind = Denominator::parse(denominator)?;
    let delim = Delim::parse(delimiter)?;
    let header_mode = Header::parse(header)?;

    if data.trim().is_empty() {
        return Err("data is empty — paste rows of numbers, one observation per line".to_string());
    }
    let parsed = parse_matrix(data, delim, header_mode)?;
    let rows = parsed.rows;
    let n = rows.len();
    if n == 0 {
        return Err("no data rows found — paste at least 2 observations below any header row".to_string());
    }
    let k = rows[0].len();
    if k == 0 {
        return Err("no columns found in the data".to_string());
    }
    if k > MAX_COLS {
        return Err(format!("too many columns — at most {MAX_COLS} variables"));
    }
    if n < 2 {
        return Err(format!(
            "need at least 2 observations to compute a covariance, got {n}"
        ));
    }
    let names = resolve_names(parsed.header, labels, k)?;
    let w = parse_weights(weights, n, denom_kind)?;
    let weighted = w.is_some();
    let w = w.unwrap_or_else(|| vec![1.0; n]);
    let w_sum: f64 = w.iter().sum();
    let denom = match denom_kind {
        Denominator::Sample => w_sum - 1.0,
        Denominator::Population => w_sum,
    };
    if denom <= 0.0 {
        return Err("the denominator is not positive — check the weights and denominator".to_string());
    }

    // Column means, then the centered matrix both the covariance sums and the
    // centered/standardized outputs are built from.
    let mut means = vec![0.0; k];
    for (r, row) in rows.iter().enumerate() {
        for j in 0..k {
            means[j] += w[r] * row[j];
        }
    }
    for m in means.iter_mut() {
        *m /= w_sum;
    }
    let centered: Vec<Vec<f64>> = rows
        .iter()
        .map(|row| (0..k).map(|j| row[j] - means[j]).collect())
        .collect();

    let mut cov = vec![vec![0.0; k]; k];
    for i in 0..k {
        for j in i..k {
            let mut s = 0.0;
            for (r, row) in centered.iter().enumerate() {
                s += w[r] * row[i] * row[j];
            }
            let v = s / denom;
            cov[i][j] = v;
            cov[j][i] = v;
        }
    }
    let variances: Vec<f64> = (0..k).map(|i| cov[i][i]).collect();
    let std_devs: Vec<f64> = variances.iter().map(|v| v.max(0.0).sqrt()).collect();
    let total_variance: f64 = variances.iter().sum();

    if kind.needs_std_dev() {
        if let Some(i) = std_devs.iter().position(|s| *s <= 0.0) {
            return Err(format!(
                "column '{}' is constant (zero variance), so the {} matrix is undefined — drop that column or use matrix=covariance",
                names[i],
                kind.as_str()
            ));
        }
    }

    let out: Vec<Vec<f64>> = match kind {
        MatrixKind::Covariance => cov
            .iter()
            .map(|r| r.iter().map(|v| round_to(*v, decimals)).collect())
            .collect(),
        MatrixKind::Correlation => (0..k)
            .map(|i| {
                (0..k)
                    .map(|j| {
                        let c = cov[i][j] / (std_devs[i] * std_devs[j]);
                        round_to(c.clamp(-1.0, 1.0), decimals)
                    })
                    .collect()
            })
            .collect(),
        MatrixKind::Centered => centered
            .iter()
            .map(|r| r.iter().map(|v| round_to(*v, decimals)).collect())
            .collect(),
        MatrixKind::Standardized => centered
            .iter()
            .map(|r| {
                (0..k)
                    .map(|j| round_to(r[j] / std_devs[j], decimals))
                    .collect()
            })
            .collect(),
    };

    Ok(CovarianceResult {
        kind: kind.as_str().to_string(),
        denominator: denom_kind.as_str().to_string(),
        n,
        variables: k,
        variable_names: names,
        weighted,
        weight_sum: round_to(w_sum, decimals),
        means: means.iter().map(|v| round_to(*v, decimals)).collect(),
        variances: variances.iter().map(|v| round_to(*v, decimals)).collect(),
        std_devs: std_devs.iter().map(|v| round_to(*v, decimals)).collect(),
        total_variance: round_to(total_variance, decimals),
        matrix: out,
    })
}

/// Render one aligned table: the first column is left-aligned (row labels), the
/// rest right-aligned, indented two spaces like the other gizza reports.
fn render_table(head: &[String], body: &[Vec<String>]) -> String {
    let cols = head.len();
    let mut w = vec![0usize; cols];
    for (i, h) in head.iter().enumerate() {
        w[i] = h.chars().count();
    }
    for row in body {
        for (i, cell) in row.iter().enumerate() {
            w[i] = w[i].max(cell.chars().count());
        }
    }
    let line = |cells: &[String]| -> String {
        let mut s = String::from("  ");
        for (i, c) in cells.iter().enumerate() {
            if i > 0 {
                s.push_str("  ");
            }
            let pad = w[i].saturating_sub(c.chars().count());
            if i == 0 {
                s.push_str(c);
                s.push_str(&" ".repeat(pad));
            } else {
                s.push_str(&" ".repeat(pad));
                s.push_str(c);
            }
        }
        s.trim_end().to_string()
    };
    let mut out = vec![line(head)];
    for row in body {
        out.push(line(row));
    }
    out.join("\n")
}

fn title(res: &CovarianceResult, kind: MatrixKind) -> String {
    let denom_note = match (res.denominator.as_str(), res.weighted) {
        ("population", true) => "population, Σw denominator, weighted",
        ("population", false) => "population, n denominator",
        (_, true) => "sample, Σw − 1 denominator, weighted",
        _ => "sample, n − 1 denominator",
    };
    match kind {
        MatrixKind::Covariance => format!(
            "Covariance matrix ({}) — {} observations × {} variables",
            denom_note, res.n, res.variables
        ),
        MatrixKind::Correlation => format!(
            "Correlation matrix (standardized covariance) — {} observations × {} variables",
            res.n, res.variables
        ),
        MatrixKind::Centered => format!(
            "Centered data (each value minus its column mean) — {} rows × {} variables",
            res.n, res.variables
        ),
        MatrixKind::Standardized => format!(
            "Standardized data (z-scores: (value − mean) ÷ std dev) — {} rows × {} variables",
            res.n, res.variables
        ),
    }
}

fn stats_rows(res: &CovarianceResult, decimals: usize) -> (Vec<String>, Vec<Vec<String>>) {
    let head = vec![
        "variable".to_string(),
        "n".to_string(),
        "mean".to_string(),
        "variance".to_string(),
        "std dev".to_string(),
    ];
    let body = (0..res.variables)
        .map(|i| {
            vec![
                res.variable_names[i].clone(),
                res.n.to_string(),
                fmt_fixed(res.means[i], decimals),
                fmt_fixed(res.variances[i], decimals),
                fmt_fixed(res.std_devs[i], decimals),
            ]
        })
        .collect();
    (head, body)
}

fn render_text(res: &CovarianceResult, kind: MatrixKind, stats: bool, decimals: usize) -> String {
    let mut out = vec![title(res, kind), String::new()];

    let mut head = vec![if kind.is_matrix() { String::new() } else { "row".to_string() }];
    head.extend(res.variable_names.iter().cloned());
    let body: Vec<Vec<String>> = res
        .matrix
        .iter()
        .enumerate()
        .map(|(i, row)| {
            let mut cells = vec![if kind.is_matrix() {
                res.variable_names[i].clone()
            } else {
                (i + 1).to_string()
            }];
            cells.extend(row.iter().map(|v| fmt_fixed(*v, decimals)));
            cells
        })
        .collect();
    out.push(render_table(&head, &body));

    if kind == MatrixKind::Covariance {
        out.push(String::new());
        out.push(format!(
            "Total variance (trace): {}",
            fmt_fixed(res.total_variance, decimals)
        ));
    }
    if res.weighted {
        out.push(format!("Sum of weights: {}", fmt_fixed(res.weight_sum, decimals)));
    }
    if stats {
        out.push(String::new());
        out.push("Column summary:".to_string());
        let (h, b) = stats_rows(res, decimals);
        out.push(render_table(&h, &b));
    }
    out.join("\n")
}

fn md_table(head: &[String], body: &[Vec<String>]) -> String {
    let mut out = vec![
        format!("| {} |", head.join(" | ")),
        format!(
            "| {} |",
            head.iter()
                .enumerate()
                .map(|(i, _)| if i == 0 { "---" } else { "---:" })
                .collect::<Vec<_>>()
                .join(" | ")
        ),
    ];
    for row in body {
        out.push(format!("| {} |", row.join(" | ")));
    }
    out.join("\n")
}

fn render_markdown(
    res: &CovarianceResult,
    kind: MatrixKind,
    stats: bool,
    decimals: usize,
) -> String {
    let mut out = vec![format!("**{}**", title(res, kind)), String::new()];

    let mut head = vec![if kind.is_matrix() { " ".to_string() } else { "row".to_string() }];
    head.extend(res.variable_names.iter().cloned());
    let body: Vec<Vec<String>> = res
        .matrix
        .iter()
        .enumerate()
        .map(|(i, row)| {
            let mut cells = vec![if kind.is_matrix() {
                format!("**{}**", res.variable_names[i])
            } else {
                (i + 1).to_string()
            }];
            cells.extend(row.iter().map(|v| fmt_fixed(*v, decimals)));
            cells
        })
        .collect();
    out.push(md_table(&head, &body));

    if kind == MatrixKind::Covariance {
        out.push(String::new());
        out.push(format!(
            "Total variance (trace): {}",
            fmt_fixed(res.total_variance, decimals)
        ));
    }
    if stats {
        out.push(String::new());
        out.push("**Column summary**".to_string());
        out.push(String::new());
        let (h, b) = stats_rows(res, decimals);
        out.push(md_table(&h, &b));
    }
    out.join("\n")
}

fn render_csv(res: &CovarianceResult, kind: MatrixKind, decimals: usize) -> String {
    let mut out = Vec::new();
    let corner = if kind.is_matrix() { "" } else { "row" };
    out.push(format!(
        "{},{}",
        corner,
        res.variable_names
            .iter()
            .map(|n| csv_cell(n))
            .collect::<Vec<_>>()
            .join(",")
    ));
    for (i, row) in res.matrix.iter().enumerate() {
        let label = if kind.is_matrix() {
            csv_cell(&res.variable_names[i])
        } else {
            (i + 1).to_string()
        };
        let cells: Vec<String> = row.iter().map(|v| fmt_fixed(*v, decimals)).collect();
        out.push(format!("{},{}", label, cells.join(",")));
    }
    out.join("\n")
}

fn csv_cell(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// Entry point shared by the chat block, the CLI and the browser page.
#[allow(clippy::too_many_arguments)]
pub fn run(
    data: &str,
    labels: &str,
    delimiter: &str,
    header: &str,
    matrix: &str,
    denominator: &str,
    weights: &str,
    decimals: f64,
    stats: bool,
    format: &str,
) -> Result<String, String> {
    let res = compute(
        data,
        labels,
        delimiter,
        header,
        matrix,
        denominator,
        weights,
        decimals,
    )?;
    let kind = MatrixKind::parse(matrix)?;
    let decimals = decimals as usize;
    match format.trim().to_ascii_lowercase().as_str() {
        "" | "text" => Ok(render_text(&res, kind, stats, decimals)),
        "markdown" | "md" => Ok(render_markdown(&res, kind, stats, decimals)),
        "csv" => Ok(render_csv(&res, kind, decimals)),
        "json" => serde_json::to_string_pretty(&res).map_err(|e| e.to_string()),
        other => Err(format!(
            "unknown format '{other}' — use text, markdown, csv or json"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DATA: &str = "height,weight,age\n170,65,30\n180,80,42\n165,59,25\n175,72,35\n190,95,50\n160,54,22";

    fn run_default(data: &str) -> Result<String, String> {
        run(data, "", "auto", "auto", "covariance", "sample", "", 6.0, true, "text")
    }

    #[test]
    fn sample_covariance_matches_hand_computed_values() {
        // x = 1,2,3,4,5 ; y = 2,4,5,4,5 → var(x) = 2.5, cov(x,y) = 1.25,
        // var(y) = 1.5 with the n−1 denominator.
        let out = run(
            "x,y\n1,2\n2,4\n3,5\n4,4\n5,5",
            "",
            "auto",
            "auto",
            "covariance",
            "sample",
            "",
            4.0,
            true,
            "json",
        )
        .unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["matrix"][0][0], 2.5);
        assert_eq!(v["matrix"][0][1], 1.5);
        assert_eq!(v["matrix"][1][0], 1.5);
        assert_eq!(v["matrix"][1][1], 1.5);
        assert_eq!(v["means"][0], 3.0);
        assert_eq!(v["means"][1], 4.0);
        assert_eq!(v["n"], 5);
        assert_eq!(v["variable_names"][0], "x");
        assert_eq!(v["total_variance"], 4.0);
    }

    #[test]
    fn population_denominator_scales_by_n_over_n_minus_1() {
        let sample: serde_json::Value = serde_json::from_str(
            &run("1,2\n2,4\n3,5\n4,4\n5,5", "", "auto", "auto", "covariance", "sample", "", 6.0, false, "json").unwrap(),
        )
        .unwrap();
        let pop: serde_json::Value = serde_json::from_str(
            &run("1,2\n2,4\n3,5\n4,4\n5,5", "", "auto", "auto", "covariance", "population", "", 6.0, false, "json").unwrap(),
        )
        .unwrap();
        assert_eq!(sample["matrix"][0][0], 2.5);
        assert_eq!(pop["matrix"][0][0], 2.0); // 2.5 × 4/5
        assert_eq!(pop["denominator"], "population");
        // No header row → default v1, v2 names.
        assert_eq!(pop["variable_names"][0], "v1");
    }

    #[test]
    fn correlation_is_scale_free_and_unit_diagonal() {
        let out = run(DATA, "", "auto", "auto", "correlation", "sample", "", 6.0, false, "json").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["matrix"][0][0], 1.0);
        assert_eq!(v["matrix"][1][1], 1.0);
        assert_eq!(v["matrix"][0][1], v["matrix"][1][0]);
        let r = v["matrix"][0][1].as_f64().unwrap();
        assert!(r > 0.98 && r <= 1.0, "height/weight correlate strongly: {r}");
        // The population denominator cancels out of a correlation.
        let pop: serde_json::Value = serde_json::from_str(
            &run(DATA, "", "auto", "auto", "correlation", "population", "", 6.0, false, "json").unwrap(),
        )
        .unwrap();
        assert_eq!(v["matrix"], pop["matrix"]);
    }

    #[test]
    fn centered_and_standardized_return_the_data_matrix() {
        let centered: serde_json::Value = serde_json::from_str(
            &run("x,y\n1,2\n2,4\n3,5\n4,4\n5,5", "", "auto", "auto", "centered", "sample", "", 6.0, false, "json").unwrap(),
        )
        .unwrap();
        assert_eq!(centered["matrix"].as_array().unwrap().len(), 5);
        assert_eq!(centered["matrix"][0][0], -2.0);
        assert_eq!(centered["matrix"][4][0], 2.0);

        let z: serde_json::Value = serde_json::from_str(
            &run("x,y\n1,2\n2,4\n3,5\n4,4\n5,5", "", "auto", "auto", "standardized", "sample", "", 6.0, false, "json").unwrap(),
        )
        .unwrap();
        // z = (x − 3) / √2.5
        let expect = -2.0 / 2.5f64.sqrt();
        assert!((z["matrix"][0][0].as_f64().unwrap() - expect).abs() < 1e-6);
        // Column mean of the z-scores is 0.
        let col: f64 = (0..5).map(|i| z["matrix"][i][0].as_f64().unwrap()).sum();
        assert!(col.abs() < 1e-6);
    }

    #[test]
    fn weights_reproduce_repeated_rows() {
        // One row weighted 2 == that row written twice (frequency weights).
        let weighted = run("1,2\n2,4\n3,5", "", "auto", "no", "covariance", "sample", "2,1,1", 6.0, false, "json").unwrap();
        let repeated = run("1,2\n1,2\n2,4\n3,5", "", "auto", "no", "covariance", "sample", "", 6.0, false, "json").unwrap();
        let a: serde_json::Value = serde_json::from_str(&weighted).unwrap();
        let b: serde_json::Value = serde_json::from_str(&repeated).unwrap();
        assert_eq!(a["matrix"], b["matrix"]);
        assert_eq!(a["means"], b["means"]);
        assert_eq!(a["weighted"], true);
        assert_eq!(a["weight_sum"], 4.0);
    }

    #[test]
    fn delimiters_and_header_modes_parse() {
        let tab = run("a\tb\n1\t2\n3\t4\n5\t7", "", "tab", "auto", "covariance", "sample", "", 4.0, false, "csv").unwrap();
        assert!(tab.starts_with(",a,b"), "{tab}");
        let semi = run("1;2\n3;4\n5;7", "", "semicolon", "no", "covariance", "sample", "", 4.0, false, "csv").unwrap();
        assert!(semi.starts_with(",v1,v2"), "{semi}");
        let pipe = run("1|2\n3|4\n5|7", "", "pipe", "no", "covariance", "sample", "", 4.0, false, "csv").unwrap();
        assert_eq!(semi, pipe);
        // header=yes forces a numeric first row to be treated as names.
        let forced: serde_json::Value = serde_json::from_str(
            &run("1,2\n3,4\n5,7", "", "comma", "yes", "covariance", "sample", "", 4.0, false, "json").unwrap(),
        )
        .unwrap();
        assert_eq!(forced["variable_names"][0], "1");
        assert_eq!(forced["n"], 2);
    }

    #[test]
    fn labels_override_the_header_row() {
        let v: serde_json::Value = serde_json::from_str(
            &run(DATA, "a,b,c", "auto", "auto", "covariance", "sample", "", 2.0, false, "json").unwrap(),
        )
        .unwrap();
        assert_eq!(v["variable_names"][0], "a");
        assert_eq!(v["variable_names"][2], "c");
    }

    #[test]
    fn decimals_control_the_rendered_precision() {
        let two = run(DATA, "", "auto", "auto", "covariance", "sample", "", 2.0, false, "csv").unwrap();
        // var(height) = 583.3333 / 5 = 116.6667.
        assert!(two.contains("height,116.67,"), "{two}");
        let zero = run(DATA, "", "auto", "auto", "covariance", "sample", "", 0.0, false, "csv").unwrap();
        assert!(zero.contains("height,117,"), "{zero}");
    }

    #[test]
    fn text_output_is_a_labelled_aligned_table() {
        let out = run_default(DATA).unwrap();
        assert!(out.starts_with("Covariance matrix (sample, n − 1 denominator) — 6 observations × 3 variables"));
        assert!(out.contains("Total variance (trace): 454.433333"), "{out}");
        assert!(out.contains("Column summary:"));
        assert!(out.contains("height"));
    }

    #[test]
    fn markdown_output_is_a_pipe_table() {
        let out = run(DATA, "", "auto", "auto", "covariance", "sample", "", 3.0, true, "markdown").unwrap();
        assert!(out.contains("| **height** |"), "{out}");
        assert!(out.contains("| ---: |"));
        assert!(out.contains("**Column summary**"));
    }

    #[test]
    fn empty_input_is_rejected() {
        let err = run_default("   \n  ").unwrap_err();
        assert!(err.contains("data is empty"), "{err}");
    }

    #[test]
    fn single_observation_is_rejected() {
        let err = run_default("x,y\n1,2").unwrap_err();
        assert!(err.contains("at least 2 observations"), "{err}");
    }

    #[test]
    fn ragged_rows_are_rejected() {
        let err = run_default("1,2\n3,4,5").unwrap_err();
        assert!(err.contains("same number of columns"), "{err}");
    }

    #[test]
    fn non_numeric_cells_are_rejected() {
        let err = run_default("x,y\n1,2\n3,oops").unwrap_err();
        assert!(err.contains("'oops' is not a number"), "{err}");
    }

    #[test]
    fn constant_column_is_rejected_for_correlation_only() {
        let err = run("x,y\n1,5\n2,5\n3,5", "", "auto", "auto", "correlation", "sample", "", 6.0, false, "text")
            .unwrap_err();
        assert!(err.contains("column 'y' is constant"), "{err}");
        // …but a constant column is fine for a plain covariance matrix.
        let ok = run("x,y\n1,5\n2,5\n3,5", "", "auto", "auto", "covariance", "sample", "", 6.0, false, "csv").unwrap();
        assert!(ok.contains("y,0.000000,0.000000"), "{ok}");
    }

    #[test]
    fn bad_enums_and_caps_are_rejected() {
        assert!(run(DATA, "", "auto", "auto", "cov", "sample", "", 6.0, true, "xml")
            .unwrap_err()
            .contains("unknown format"));
        assert!(run(DATA, "", "auto", "auto", "eigen", "sample", "", 6.0, true, "text")
            .unwrap_err()
            .contains("unknown matrix"));
        assert!(run(DATA, "", "auto", "auto", "covariance", "half", "", 6.0, true, "text")
            .unwrap_err()
            .contains("unknown denominator"));
        assert!(run(DATA, "", "colon", "auto", "covariance", "sample", "", 6.0, true, "text")
            .unwrap_err()
            .contains("unknown delimiter"));
        assert!(run(DATA, "", "auto", "maybe", "covariance", "sample", "", 6.0, true, "text")
            .unwrap_err()
            .contains("unknown header"));
        // decimals boundary: 12 is accepted, 13 is not.
        assert!(run(DATA, "", "auto", "auto", "covariance", "sample", "", 12.0, false, "csv").is_ok());
        assert!(run(DATA, "", "auto", "auto", "covariance", "sample", "", 13.0, false, "csv")
            .unwrap_err()
            .contains("between 0 and 12"));
        assert!(run(DATA, "", "auto", "auto", "covariance", "sample", "", 2.5, false, "csv")
            .unwrap_err()
            .contains("whole number"));
    }

    #[test]
    fn bad_weights_are_rejected() {
        assert!(run("1,2\n3,4\n5,7", "", "auto", "no", "covariance", "sample", "1,2", 6.0, false, "text")
            .unwrap_err()
            .contains("one weight per row"));
        assert!(run("1,2\n3,4\n5,7", "", "auto", "no", "covariance", "sample", "1,-2,1", 6.0, false, "text")
            .unwrap_err()
            .contains("finite weight"));
        assert!(run("1,2\n3,4\n5,7", "", "auto", "no", "covariance", "sample", "0,0,0", 6.0, false, "text")
            .unwrap_err()
            .contains("not all be zero"));
    }

    #[test]
    fn column_and_row_caps_are_enforced() {
        let wide_row = (1..=MAX_COLS + 1)
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let err = run_default(&format!("{wide_row}\n{wide_row}")).unwrap_err();
        assert!(err.contains("at most 100 variables"), "{err}");

        let tall = (0..MAX_ROWS + 1)
            .map(|i| format!("{i},{i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let err = run_default(&tall).unwrap_err();
        assert!(err.contains("at most 20000 observations"), "{err}");
    }

    #[test]
    fn labels_count_mismatch_is_rejected() {
        let err = run(DATA, "a,b", "auto", "auto", "covariance", "sample", "", 6.0, true, "text").unwrap_err();
        assert!(err.contains("got 2 names but the data has 3 columns"), "{err}");
    }
}
