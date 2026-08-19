//! pca-visualizer core — reduce a high-dimensional table to two dimensions with
//! **PCA** or **t-SNE** and draw the projection as a labelled SVG scatter plot,
//! one point per row, coloured by a label column.
//!
//! Everything is pure Rust and **deterministic**: the PCA half delegates to the
//! sibling `principal-component-analysis` core (cyclic Jacobi eigen-decomposition,
//! sign-fixed components) so both tools report identical numbers, and the t-SNE
//! half uses a PCA initialisation instead of a random one — there is no RNG
//! anywhere, so the same input always produces byte-identical output on every
//! backend (wasmi, wasm32, native).
//!
//! Input is a delimited table: one observation per row, one variable per column,
//! split on commas, tabs, semicolons, pipes or whitespace. A leading header row
//! is detected automatically. One non-numeric column may carry a class/group
//! label; it is dropped from the maths and used to colour the points and build
//! the legend.

use serde::Serialize;

/// Maximum number of observations (rows) accepted for PCA.
pub const MAX_ROWS: usize = 5_000;
/// Maximum number of numeric variables (columns) accepted.
pub const MAX_COLS: usize = 100;
/// Maximum number of observations accepted for t-SNE — it is O(n²) in both time
/// and memory per iteration, so the cap is far lower than the PCA one.
pub const MAX_TSNE_ROWS: usize = 1_000;
/// Maximum number of points that may carry a drawn text label (`show_labels`).
pub const MAX_LABEL_POINTS: usize = 200;
/// How many legend entries are drawn before the rest are summarised as "+N more".
pub const MAX_LEGEND_ENTRIES: usize = 14;

/// Categorical point colours, cycled in first-seen label order.
const PALETTE: [&str; 10] = [
    "#2563eb", "#dc2626", "#16a34a", "#f59e0b", "#7c3aed", "#0891b2", "#db2777", "#65a30d",
    "#ea580c", "#475569",
];
/// Colour used when the data has no label column (a single ungrouped series).
const SINGLE_COLOR: &str = "#2563eb";

/// Which 2-D projection to compute.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    /// Principal component analysis — linear, deterministic, preserves global structure.
    Pca,
    /// t-distributed stochastic neighbor embedding — non-linear, preserves local neighbourhoods.
    Tsne,
}

impl Method {
    /// Parse the `method` parameter (case-insensitive; `t-sne` and `tsne` both work).
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().replace('-', "").as_str() {
            "" | "pca" => Ok(Method::Pca),
            "tsne" => Ok(Method::Tsne),
            other => Err(format!("unknown method '{other}' — use 'pca' or 'tsne'")),
        }
    }
}

/// Output representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// A standalone SVG scatter plot.
    Svg,
    /// `index,label,x,y` rows — the coordinates, ready for another chart tool.
    Csv,
    /// The full projection as JSON (coordinates, categories, explained variance).
    Json,
}

impl Format {
    /// Parse the `format` parameter (case-insensitive).
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "svg" => Ok(Format::Svg),
            "csv" => Ok(Format::Csv),
            "json" => Ok(Format::Json),
            other => Err(format!("unknown format '{other}' — use 'svg', 'csv' or 'json'")),
        }
    }
}

/// Every knob the projection and the plot expose.
#[derive(Debug, Clone)]
pub struct Options {
    /// PCA or t-SNE.
    pub method: Method,
    /// Header name or 1-based index of the column holding each row's class label.
    /// Empty auto-detects the single non-numeric column, if there is one.
    pub label_column: String,
    /// Standardize every numeric column to unit variance before projecting.
    pub scale: bool,
    /// t-SNE perplexity — roughly the neighbourhood size each point preserves.
    pub perplexity: f64,
    /// t-SNE gradient-descent iterations.
    pub iterations: u32,
    /// t-SNE learning rate.
    pub learning_rate: f64,
    /// Draw each point's label text next to its marker.
    pub show_labels: bool,
    /// Marker radius in pixels.
    pub point_size: f64,
    /// Optional chart title.
    pub title: String,
    /// SVG width in pixels.
    pub width: u32,
    /// SVG height in pixels.
    pub height: u32,
    /// Output representation.
    pub format: Format,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            method: Method::Pca,
            label_column: String::new(),
            scale: true,
            perplexity: 30.0,
            iterations: 500,
            learning_rate: 200.0,
            show_labels: false,
            point_size: 4.0,
            title: String::new(),
            width: 720,
            height: 520,
            format: Format::Svg,
        }
    }
}

/// One plotted observation.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Point {
    /// 1-based row number in the input data (header excluded).
    pub index: usize,
    /// The row's class label, or an empty string when the data has no label column.
    pub label: String,
    /// First projected coordinate (PC1, or t-SNE dimension 1).
    pub x: f64,
    /// Second projected coordinate (PC2, or t-SNE dimension 2).
    pub y: f64,
}

/// The finished 2-D projection.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Projection {
    /// `"pca"` or `"tsne"`.
    pub method: String,
    /// Number of observations projected.
    pub n: usize,
    /// Number of numeric variables that fed the projection.
    pub variables: usize,
    /// Names of those variables, in column order.
    pub variable_names: Vec<String>,
    /// Name of the column used for colouring, or an empty string when there is none.
    pub label_name: String,
    /// Distinct labels in first-seen order — the legend, in colour order.
    pub categories: Vec<String>,
    /// Axis caption for the horizontal axis.
    pub x_axis: String,
    /// Axis caption for the vertical axis.
    pub y_axis: String,
    /// Share of the total variance carried by each axis (PCA only; `None` for t-SNE).
    pub explained_variance: Option<Vec<f64>>,
    /// The perplexity actually used, after clamping to the row count (t-SNE only).
    pub perplexity_used: Option<f64>,
    /// The projected points, in input row order.
    pub points: Vec<Point>,
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/// Pick the column delimiter from the first populated line: the first of
/// comma / tab / semicolon / pipe that appears, else whitespace.
fn detect_delimiter(line: &str) -> char {
    for c in [',', '\t', ';', '|'] {
        if line.contains(c) {
            return c;
        }
    }
    ' '
}

fn split_row(line: &str, delim: char) -> Vec<String> {
    if delim == ' ' {
        line.split_whitespace().map(|s| s.to_string()).collect()
    } else {
        line.split(delim)
            .map(|s| s.trim().trim_matches('"').trim().to_string())
            .collect()
    }
}

/// A finite number, or `None` for anything else (text, blanks, `NaN`, `inf`).
fn numeric(field: &str) -> Option<f64> {
    field.trim().parse::<f64>().ok().filter(|v| v.is_finite())
}

struct Table {
    header: Option<Vec<String>>,
    rows: Vec<Vec<String>>,
}

fn parse_table(data: &str) -> Result<Table, String> {
    let lines: Vec<&str> = data
        .lines()
        .map(|l| l.trim_end_matches('\r').trim())
        .filter(|l| !l.is_empty())
        .collect();
    if lines.is_empty() {
        return Err("no data — paste a table with one observation per line".into());
    }
    let delim = detect_delimiter(lines[0]);
    let mut rows: Vec<Vec<String>> = lines.iter().map(|l| split_row(l, delim)).collect();

    let ncol = rows[0].len();
    if ncol < 2 {
        return Err(
            "each row needs at least 2 columns — separate values with commas, tabs, semicolons, pipes or spaces"
                .into(),
        );
    }
    for (i, r) in rows.iter().enumerate() {
        if r.len() != ncol {
            return Err(format!(
                "row {} has {} columns but row 1 has {ncol} — every row needs the same number of values",
                i + 1,
                r.len()
            ));
        }
    }

    // A first row is a header when it carries strictly fewer numeric fields than
    // the row below it (a header of names has none; a data row has some).
    let count_numeric = |r: &Vec<String>| r.iter().filter(|f| numeric(f).is_some()).count();
    let header = if rows.len() >= 2 && count_numeric(&rows[0]) < count_numeric(&rows[1]) {
        Some(rows.remove(0))
    } else {
        None
    };
    if rows.is_empty() {
        return Err("the data has a header row but no observations under it".into());
    }
    Ok(Table { header, rows })
}

/// The numeric matrix plus the per-row labels, after the label column is split off.
struct Dataset {
    names: Vec<String>,
    label_name: String,
    labels: Vec<String>,
    matrix: Vec<Vec<f64>>,
}

/// Resolve `label_column` (header name or 1-based index) to a column index.
fn resolve_label_column(
    spec: &str,
    header: &Option<Vec<String>>,
    ncol: usize,
    text_columns: &[usize],
) -> Result<Option<usize>, String> {
    let spec = spec.trim();
    if spec.is_empty() {
        // Auto: the single non-numeric column is the label, if there is exactly one.
        return Ok(text_columns.first().copied());
    }
    if let Some(h) = header {
        if let Some(i) = h
            .iter()
            .position(|name| name.trim().eq_ignore_ascii_case(spec))
        {
            return Ok(Some(i));
        }
    }
    if let Ok(i) = spec.parse::<usize>() {
        if i >= 1 && i <= ncol {
            return Ok(Some(i - 1));
        }
        return Err(format!(
            "label_column index {i} is out of range — the data has {ncol} columns"
        ));
    }
    match header {
        Some(h) => Err(format!(
            "no column named '{spec}' — available columns: {}",
            h.join(", ")
        )),
        None => Err(format!(
            "no column named '{spec}' — the data has no header row, so use a 1-based column index (1–{ncol})"
        )),
    }
}

fn build_dataset(data: &str, label_column: &str) -> Result<Dataset, String> {
    let Table { header, rows } = parse_table(data)?;
    let ncol = rows[0].len();

    // Columns that are not fully numeric can only be labels, never variables.
    let text_columns: Vec<usize> = (0..ncol)
        .filter(|j| rows.iter().any(|r| numeric(&r[*j]).is_none()))
        .collect();
    let label_idx = resolve_label_column(label_column, &header, ncol, &text_columns)?;

    let names: Vec<String> = match &header {
        Some(h) => h.iter().map(|s| s.trim().to_string()).collect(),
        None => (1..=ncol).map(|i| format!("v{i}")).collect(),
    };

    if let Some(bad) = text_columns.iter().find(|j| Some(**j) != label_idx) {
        let sample = rows
            .iter()
            .find(|r| numeric(&r[*bad]).is_none())
            .map(|r| r[*bad].clone())
            .unwrap_or_default();
        return Err(format!(
            "column '{}' is not numeric (found \"{}\") — remove it, or set label_column to it to colour the points by it",
            names[*bad], sample
        ));
    }

    let feature_idx: Vec<usize> = (0..ncol).filter(|j| Some(*j) != label_idx).collect();
    if feature_idx.len() < 2 {
        return Err(format!(
            "need at least 2 numeric variables to project onto 2 dimensions; got {}",
            feature_idx.len()
        ));
    }
    if feature_idx.len() > MAX_COLS {
        return Err(format!(
            "too many variables: {} (limit {MAX_COLS})",
            feature_idx.len()
        ));
    }
    if rows.len() > MAX_ROWS {
        return Err(format!(
            "too many observations: {} (limit {MAX_ROWS})",
            rows.len()
        ));
    }
    if rows.len() < 3 {
        return Err(format!(
            "need at least 3 observations (rows) to project; got {}",
            rows.len()
        ));
    }

    let matrix: Vec<Vec<f64>> = rows
        .iter()
        .map(|r| feature_idx.iter().map(|j| numeric(&r[*j]).unwrap()).collect())
        .collect();
    let labels: Vec<String> = match label_idx {
        Some(j) => rows.iter().map(|r| r[j].trim().to_string()).collect(),
        None => vec![String::new(); rows.len()],
    };

    Ok(Dataset {
        names: feature_idx.iter().map(|j| names[*j].clone()).collect(),
        label_name: label_idx.map(|j| names[j].clone()).unwrap_or_default(),
        labels,
        matrix,
    })
}

// ---------------------------------------------------------------------------
// Projections
// ---------------------------------------------------------------------------

/// Column names, made safe to pass through the sibling PCA core's `labels`
/// argument (which splits on `,`, `;` and tab).
fn sanitized_names(names: &[String]) -> String {
    names
        .iter()
        .map(|n| {
            let cleaned: String = n
                .chars()
                .map(|c| if c == ',' || c == ';' || c == '\t' { ' ' } else { c })
                .collect();
            let cleaned = cleaned.trim().to_string();
            if cleaned.is_empty() {
                "?".to_string()
            } else {
                cleaned
            }
        })
        .collect::<Vec<_>>()
        .join(",")
}

/// Run PCA via the sibling block's engine and return `(scores, proportions)`.
fn pca_scores(ds: &Dataset, scale: bool) -> Result<(Vec<[f64; 2]>, Vec<f64>), String> {
    let csv: String = ds
        .matrix
        .iter()
        .map(|row| {
            row.iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join(",")
        })
        .collect::<Vec<_>>()
        .join("\n");
    let pca = gizza_ai_principal_component_analysis_core::analyze(
        &csv,
        &sanitized_names(&ds.names),
        2,
        scale,
    )?;
    let scores: Vec<[f64; 2]> = pca
        .scores
        .iter()
        .map(|s| [*s.first().unwrap_or(&0.0), *s.get(1).unwrap_or(&0.0)])
        .collect();
    let proportions: Vec<f64> = pca.components.iter().map(|c| c.proportion).collect();
    Ok((scores, proportions))
}

/// Mean-centre (and optionally standardize) every column.
fn standardize(matrix: &[Vec<f64>], scale: bool) -> Vec<Vec<f64>> {
    let n = matrix.len();
    let p = matrix[0].len();
    let mut out = matrix.to_vec();
    for j in 0..p {
        let mean = matrix.iter().map(|r| r[j]).sum::<f64>() / n as f64;
        let sd = if scale && n > 1 {
            let ss: f64 = matrix.iter().map(|r| (r[j] - mean).powi(2)).sum();
            (ss / (n - 1) as f64).sqrt()
        } else {
            1.0
        };
        let sd = if sd > 1e-12 { sd } else { 1.0 };
        for row in out.iter_mut() {
            row[j] = (row[j] - mean) / sd;
        }
    }
    out
}

/// Pairwise squared Euclidean distances, row-major `n × n`.
fn squared_distances(x: &[Vec<f64>]) -> Vec<f64> {
    let n = x.len();
    let mut d = vec![0.0f64; n * n];
    for i in 0..n {
        for j in (i + 1)..n {
            let dist: f64 = x[i]
                .iter()
                .zip(&x[j])
                .map(|(a, b)| (a - b) * (a - b))
                .sum();
            d[i * n + j] = dist;
            d[j * n + i] = dist;
        }
    }
    d
}

/// Symmetric joint probabilities `P` for the requested perplexity, found per row
/// by bisecting the Gaussian precision until the conditional entropy matches.
fn joint_probabilities(d2: &[f64], n: usize, perplexity: f64) -> Vec<f64> {
    let target = perplexity.ln();
    let mut cond = vec![0.0f64; n * n];
    let mut row = vec![0.0f64; n];
    for i in 0..n {
        let (mut lo, mut hi) = (f64::NEG_INFINITY, f64::INFINITY);
        let mut beta = 1.0f64;
        for _ in 0..64 {
            let mut sum = 0.0;
            let mut weighted = 0.0;
            for j in 0..n {
                if i == j {
                    row[j] = 0.0;
                    continue;
                }
                let v = (-d2[i * n + j] * beta).exp();
                row[j] = v;
                sum += v;
                weighted += d2[i * n + j] * v;
            }
            let sum = sum.max(1e-300);
            let entropy = beta * weighted / sum + sum.ln();
            if (entropy - target).abs() < 1e-5 {
                break;
            }
            if entropy > target {
                lo = beta;
                beta = if hi.is_infinite() { beta * 2.0 } else { (beta + hi) / 2.0 };
            } else {
                hi = beta;
                beta = if lo.is_infinite() { beta / 2.0 } else { (beta + lo) / 2.0 };
            }
        }
        let sum: f64 = row.iter().sum::<f64>().max(1e-300);
        for j in 0..n {
            cond[i * n + j] = row[j] / sum;
        }
    }
    let denom = 2.0 * n as f64;
    let mut p = vec![0.0f64; n * n];
    for i in 0..n {
        for j in 0..n {
            if i != j {
                p[i * n + j] = ((cond[i * n + j] + cond[j * n + i]) / denom).max(1e-12);
            }
        }
    }
    p
}

/// t-SNE gradient descent with early exaggeration, momentum and per-parameter
/// gains — the standard Van der Maaten schedule, started from the PCA solution
/// so the result is fully deterministic (no RNG on any backend).
fn tsne(
    ds: &Dataset,
    init: &[[f64; 2]],
    scale: bool,
    perplexity: f64,
    iterations: u32,
    learning_rate: f64,
) -> Vec<[f64; 2]> {
    let n = ds.matrix.len();
    let z = standardize(&ds.matrix, scale);
    let d2 = squared_distances(&z);
    let p = joint_probabilities(&d2, n, perplexity);

    // Scale the PCA initialisation down to the ~1e-4 spread t-SNE expects.
    let mean_x = init.iter().map(|y| y[0]).sum::<f64>() / n as f64;
    let sd_x = (init.iter().map(|y| (y[0] - mean_x).powi(2)).sum::<f64>() / n as f64).sqrt();
    let unit = if sd_x > 1e-12 { 1e-4 / sd_x } else { 1e-4 };
    let mut y: Vec<[f64; 2]> = init.iter().map(|c| [c[0] * unit, c[1] * unit]).collect();

    let mut grad = vec![[0.0f64; 2]; n];
    let mut velocity = vec![[0.0f64; 2]; n];
    let mut gains = vec![[1.0f64; 2]; n];
    let mut num = vec![0.0f64; n * n];

    for iter in 0..iterations {
        let exaggeration = if iter < 250 { 12.0 } else { 1.0 };
        let mut total = 0.0;
        for i in 0..n {
            for j in (i + 1)..n {
                let dx = y[i][0] - y[j][0];
                let dy = y[i][1] - y[j][1];
                let q = 1.0 / (1.0 + dx * dx + dy * dy);
                num[i * n + j] = q;
                num[j * n + i] = q;
                total += 2.0 * q;
            }
        }
        let total = total.max(1e-300);
        for i in 0..n {
            let (mut gx, mut gy) = (0.0, 0.0);
            for j in 0..n {
                if i == j {
                    continue;
                }
                let nij = num[i * n + j];
                let mult = 4.0 * (p[i * n + j] * exaggeration - nij / total) * nij;
                gx += mult * (y[i][0] - y[j][0]);
                gy += mult * (y[i][1] - y[j][1]);
            }
            grad[i] = [gx, gy];
        }
        let momentum = if iter < 250 { 0.5 } else { 0.8 };
        for i in 0..n {
            for d in 0..2 {
                gains[i][d] = if (grad[i][d] > 0.0) != (velocity[i][d] > 0.0) {
                    gains[i][d] + 0.2
                } else {
                    gains[i][d] * 0.8
                };
                if gains[i][d] < 0.01 {
                    gains[i][d] = 0.01;
                }
                velocity[i][d] = momentum * velocity[i][d] - learning_rate * gains[i][d] * grad[i][d];
                y[i][d] += velocity[i][d];
            }
        }
        // Re-centre so the embedding does not drift away from the origin.
        for d in 0..2 {
            let mean = y.iter().map(|c| c[d]).sum::<f64>() / n as f64;
            for c in y.iter_mut() {
                c[d] -= mean;
            }
        }
    }
    y
}

/// Compute the 2-D projection described by `opts`, without rendering it.
pub fn project(data: &str, opts: &Options) -> Result<Projection, String> {
    let ds = build_dataset(data, &opts.label_column)?;
    let n = ds.matrix.len();

    let (coords, explained, perplexity_used, x_axis, y_axis) = match opts.method {
        Method::Pca => {
            let (scores, proportions) = pca_scores(&ds, opts.scale)?;
            let pct = |i: usize| proportions.get(i).copied().unwrap_or(0.0) * 100.0;
            (
                scores,
                Some(proportions.iter().take(2).copied().collect::<Vec<f64>>()),
                None,
                format!("PC1 — {:.1}% of variance", pct(0)),
                format!("PC2 — {:.1}% of variance", pct(1)),
            )
        }
        Method::Tsne => {
            if n > MAX_TSNE_ROWS {
                return Err(format!(
                    "t-SNE is limited to {MAX_TSNE_ROWS} observations (got {n}) because it compares every pair of points — use method=pca for larger tables"
                ));
            }
            if !(1.0..=100.0).contains(&opts.perplexity) {
                return Err("perplexity must be between 1 and 100".into());
            }
            if opts.iterations < 50 || opts.iterations > 2000 {
                return Err("iterations must be between 50 and 2000".into());
            }
            if !(1.0..=1000.0).contains(&opts.learning_rate) {
                return Err("learning_rate must be between 1 and 1000".into());
            }
            // Perplexity must stay well under the sample size or the entropy
            // search cannot converge; clamp instead of failing.
            let perp = opts.perplexity.min(((n - 1) as f64 / 3.0).max(1.0));
            let (init, _) = pca_scores(&ds, opts.scale)?;
            let embedded = tsne(&ds, &init, opts.scale, perp, opts.iterations, opts.learning_rate);
            (
                embedded,
                None,
                Some(perp),
                "t-SNE dimension 1".to_string(),
                "t-SNE dimension 2".to_string(),
            )
        }
    };

    let mut categories: Vec<String> = Vec::new();
    for l in &ds.labels {
        if !l.is_empty() && !categories.iter().any(|c| c == l) {
            categories.push(l.clone());
        }
    }

    let points = (0..n)
        .map(|i| Point {
            index: i + 1,
            label: ds.labels[i].clone(),
            x: coords[i][0],
            y: coords[i][1],
        })
        .collect();

    Ok(Projection {
        method: match opts.method {
            Method::Pca => "pca".into(),
            Method::Tsne => "tsne".into(),
        },
        n,
        variables: ds.names.len(),
        variable_names: ds.names,
        label_name: ds.label_name,
        categories,
        x_axis,
        y_axis,
        explained_variance: explained,
        perplexity_used,
        points,
    })
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Short, stable tick text — plain decimals near 1, scientific notation far from it.
fn fmt_tick(v: f64) -> String {
    let a = v.abs();
    if a >= 100_000.0 || (a > 0.0 && a < 0.001) {
        format!("{v:.2e}")
    } else if a >= 100.0 {
        format!("{v:.0}")
    } else if a >= 1.0 {
        format!("{v:.2}")
    } else {
        format!("{v:.3}")
    }
}

/// Coordinate with a fixed 2-decimal precision, so the SVG is byte-stable.
fn px(v: f64) -> String {
    let r = (v * 100.0).round() / 100.0;
    let s = format!("{r:.2}");
    // Trim the trailing ".00"/"0" so the markup stays compact and readable.
    let s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    if s == "-0" {
        "0".to_string()
    } else {
        s
    }
}

fn render_svg(proj: &Projection, opts: &Options) -> Result<String, String> {
    if !(300..=2000).contains(&opts.width) {
        return Err("width must be between 300 and 2000 pixels".into());
    }
    if !(200..=2000).contains(&opts.height) {
        return Err("height must be between 200 and 2000 pixels".into());
    }
    if !(1.0..=20.0).contains(&opts.point_size) {
        return Err("point_size must be between 1 and 20 pixels".into());
    }
    if opts.show_labels && proj.n > MAX_LABEL_POINTS {
        return Err(format!(
            "show_labels draws text next to every point and is limited to {MAX_LABEL_POINTS} of them (got {}) — turn it off for larger tables",
            proj.n
        ));
    }

    let w = opts.width as f64;
    let h = opts.height as f64;
    let has_title = !opts.title.trim().is_empty();
    let legend_w = if proj.categories.is_empty() {
        0.0
    } else {
        let longest = proj
            .categories
            .iter()
            .take(MAX_LEGEND_ENTRIES)
            .map(|c| c.chars().count())
            .max()
            .unwrap_or(0)
            .max(proj.label_name.chars().count());
        (longest as f64 * 6.6 + 32.0).clamp(70.0, 220.0)
    };
    let left = 62.0;
    let right = 20.0 + legend_w;
    let top = if has_title { 48.0 } else { 22.0 };
    let bottom = 56.0;
    let plot_w = w - left - right;
    let plot_h = h - top - bottom;
    if plot_w < 80.0 || plot_h < 80.0 {
        return Err("the chart is too small for its labels — increase width/height".into());
    }

    // Data bounds with a 5% breathing margin; degenerate axes get a unit span.
    let (mut min_x, mut max_x) = (f64::INFINITY, f64::NEG_INFINITY);
    let (mut min_y, mut max_y) = (f64::INFINITY, f64::NEG_INFINITY);
    for p in &proj.points {
        min_x = min_x.min(p.x);
        max_x = max_x.max(p.x);
        min_y = min_y.min(p.y);
        max_y = max_y.max(p.y);
    }
    let pad_x = ((max_x - min_x) * 0.05).max(1e-9);
    let pad_y = ((max_y - min_y) * 0.05).max(1e-9);
    let (mut lo_x, mut hi_x) = (min_x - pad_x, max_x + pad_x);
    let (mut lo_y, mut hi_y) = (min_y - pad_y, max_y + pad_y);
    if hi_x - lo_x < 1e-12 {
        lo_x -= 0.5;
        hi_x += 0.5;
    }
    if hi_y - lo_y < 1e-12 {
        lo_y -= 0.5;
        hi_y += 0.5;
    }
    let sx = |v: f64| left + (v - lo_x) / (hi_x - lo_x) * plot_w;
    let sy = |v: f64| top + plot_h - (v - lo_y) / (hi_y - lo_y) * plot_h;

    let mut s = String::with_capacity(4096 + proj.n * 90);
    s.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"0 0 {} {}\" font-family=\"system-ui, -apple-system, Segoe UI, Roboto, sans-serif\">\n",
        opts.width, opts.height, opts.width, opts.height
    ));
    s.push_str(&format!(
        "<rect width=\"{}\" height=\"{}\" fill=\"#ffffff\"/>\n",
        opts.width, opts.height
    ));
    if has_title {
        s.push_str(&format!(
            "<text x=\"{}\" y=\"28\" text-anchor=\"middle\" font-size=\"17\" font-weight=\"600\" fill=\"#0f172a\">{}</text>\n",
            px(left + plot_w / 2.0),
            escape(opts.title.trim())
        ));
    }

    // Gridlines + tick labels: 5 divisions on each axis.
    const TICKS: usize = 5;
    for t in 0..=TICKS {
        let f = t as f64 / TICKS as f64;
        let x = left + f * plot_w;
        let vx = lo_x + f * (hi_x - lo_x);
        s.push_str(&format!(
            "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"#e2e8f0\" stroke-width=\"1\"/>\n",
            px(x),
            px(top),
            px(x),
            px(top + plot_h)
        ));
        s.push_str(&format!(
            "<text x=\"{}\" y=\"{}\" text-anchor=\"middle\" font-size=\"11\" fill=\"#64748b\">{}</text>\n",
            px(x),
            px(top + plot_h + 18.0),
            escape(&fmt_tick(vx))
        ));
        let y = top + plot_h - f * plot_h;
        let vy = lo_y + f * (hi_y - lo_y);
        s.push_str(&format!(
            "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"#e2e8f0\" stroke-width=\"1\"/>\n",
            px(left),
            px(y),
            px(left + plot_w),
            px(y)
        ));
        s.push_str(&format!(
            "<text x=\"{}\" y=\"{}\" text-anchor=\"end\" font-size=\"11\" fill=\"#64748b\">{}</text>\n",
            px(left - 8.0),
            px(y + 4.0),
            escape(&fmt_tick(vy))
        ));
    }
    // Zero axes, when the origin is inside the plotted range.
    if lo_x < 0.0 && hi_x > 0.0 {
        s.push_str(&format!(
            "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"#cbd5e1\" stroke-width=\"1\" stroke-dasharray=\"4 3\"/>\n",
            px(sx(0.0)),
            px(top),
            px(sx(0.0)),
            px(top + plot_h)
        ));
    }
    if lo_y < 0.0 && hi_y > 0.0 {
        s.push_str(&format!(
            "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"#cbd5e1\" stroke-width=\"1\" stroke-dasharray=\"4 3\"/>\n",
            px(left),
            px(sy(0.0)),
            px(left + plot_w),
            px(sy(0.0))
        ));
    }
    s.push_str(&format!(
        "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"none\" stroke=\"#94a3b8\" stroke-width=\"1\"/>\n",
        px(left),
        px(top),
        px(plot_w),
        px(plot_h)
    ));

    // Points, coloured by label.
    for p in &proj.points {
        let color = if p.label.is_empty() {
            SINGLE_COLOR
        } else {
            let idx = proj
                .categories
                .iter()
                .position(|c| *c == p.label)
                .unwrap_or(0);
            PALETTE[idx % PALETTE.len()]
        };
        s.push_str(&format!(
            "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"{}\" fill-opacity=\"0.82\" stroke=\"#ffffff\" stroke-width=\"0.8\"/>\n",
            px(sx(p.x)),
            px(sy(p.y)),
            px(opts.point_size),
            color
        ));
    }
    if opts.show_labels {
        for p in &proj.points {
            let text = if p.label.is_empty() {
                p.index.to_string()
            } else {
                p.label.clone()
            };
            s.push_str(&format!(
                "<text x=\"{}\" y=\"{}\" font-size=\"10\" fill=\"#334155\">{}</text>\n",
                px(sx(p.x) + opts.point_size + 3.0),
                px(sy(p.y) + 3.5),
                escape(&text)
            ));
        }
    }

    // Axis captions.
    s.push_str(&format!(
        "<text x=\"{}\" y=\"{}\" text-anchor=\"middle\" font-size=\"12\" fill=\"#334155\">{}</text>\n",
        px(left + plot_w / 2.0),
        px(h - 14.0),
        escape(&proj.x_axis)
    ));
    s.push_str(&format!(
        "<text x=\"16\" y=\"{}\" text-anchor=\"middle\" font-size=\"12\" fill=\"#334155\" transform=\"rotate(-90 16 {})\">{}</text>\n",
        px(top + plot_h / 2.0),
        px(top + plot_h / 2.0),
        escape(&proj.y_axis)
    ));

    // Legend.
    if !proj.categories.is_empty() {
        let lx = left + plot_w + 18.0;
        let mut ly = top + 12.0;
        if !proj.label_name.is_empty() {
            s.push_str(&format!(
                "<text x=\"{}\" y=\"{}\" font-size=\"11\" font-weight=\"600\" fill=\"#0f172a\">{}</text>\n",
                px(lx),
                px(ly),
                escape(&proj.label_name)
            ));
            ly += 16.0;
        }
        for (i, cat) in proj.categories.iter().take(MAX_LEGEND_ENTRIES).enumerate() {
            s.push_str(&format!(
                "<circle cx=\"{}\" cy=\"{}\" r=\"5\" fill=\"{}\"/>\n",
                px(lx + 5.0),
                px(ly - 4.0),
                PALETTE[i % PALETTE.len()]
            ));
            s.push_str(&format!(
                "<text x=\"{}\" y=\"{}\" font-size=\"11\" fill=\"#334155\">{}</text>\n",
                px(lx + 16.0),
                px(ly),
                escape(cat)
            ));
            ly += 17.0;
        }
        if proj.categories.len() > MAX_LEGEND_ENTRIES {
            s.push_str(&format!(
                "<text x=\"{}\" y=\"{}\" font-size=\"11\" fill=\"#64748b\">+{} more</text>\n",
                px(lx),
                px(ly),
                proj.categories.len() - MAX_LEGEND_ENTRIES
            ));
        }
    }

    s.push_str("</svg>");
    Ok(s)
}

fn render_csv(proj: &Projection) -> String {
    let (cx, cy) = if proj.method == "pca" {
        ("pc1", "pc2")
    } else {
        ("tsne1", "tsne2")
    };
    let mut out = format!("index,label,{cx},{cy}\n");
    for p in &proj.points {
        let label = if p.label.contains(',') || p.label.contains('"') {
            format!("\"{}\"", p.label.replace('"', "\"\""))
        } else {
            p.label.clone()
        };
        out.push_str(&format!("{},{},{:.6},{:.6}\n", p.index, label, p.x, p.y));
    }
    out
}

/// Project `data` and render it in the requested format.
pub fn render(data: &str, opts: &Options) -> Result<String, String> {
    let proj = project(data, opts)?;
    match opts.format {
        Format::Svg => render_svg(&proj, opts),
        Format::Csv => Ok(render_csv(&proj)),
        Format::Json => {
            serde_json::to_string_pretty(&proj).map_err(|e| format!("could not serialize: {e}"))
        }
    }
}

/// A numeric argument that has to be a whole number in `[lo, hi]`.
fn whole(v: f64, what: &str, lo: u32, hi: u32) -> Result<u32, String> {
    if !v.is_finite() || v.fract() != 0.0 {
        return Err(format!("{what} must be a whole number, got {v}"));
    }
    if v < lo as f64 || v > hi as f64 {
        return Err(format!("{what} must be between {lo} and {hi}, got {v}"));
    }
    Ok(v as u32)
}

/// Flat entry point for the chat block, the CLI and the browser wrapper — they
/// all hand over loose scalars, so the enums are parsed and the numbers are
/// range-checked here before the shared [`Options`] / [`render`] pair runs.
#[allow(clippy::too_many_arguments)]
pub fn run(
    data: &str,
    method: &str,
    label_column: &str,
    scale: bool,
    perplexity: f64,
    iterations: f64,
    learning_rate: f64,
    show_labels: bool,
    point_size: f64,
    title: &str,
    width: f64,
    height: f64,
    format: &str,
) -> Result<String, String> {
    if !perplexity.is_finite() {
        return Err("perplexity must be a number".into());
    }
    if !learning_rate.is_finite() {
        return Err("learning_rate must be a number".into());
    }
    if !point_size.is_finite() {
        return Err("point_size must be a number".into());
    }
    let opts = Options {
        method: Method::parse(method)?,
        label_column: label_column.to_string(),
        scale,
        perplexity,
        iterations: whole(iterations, "iterations", 50, 2000)?,
        learning_rate,
        show_labels,
        point_size,
        title: title.to_string(),
        width: whole(width, "width", 300, 2000)?,
        height: whole(height, "height", 200, 2000)?,
        format: Format::parse(format)?,
    };
    render(data, &opts)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Three tight clusters in 4 dimensions, with a class column.
    fn sample() -> String {
        let mut s = String::from("f1,f2,f3,f4,species\n");
        for (k, name) in ["alpha", "beta", "gamma"].iter().enumerate() {
            for i in 0..6 {
                let o = k as f64 * 10.0;
                let j = i as f64 * 0.3;
                s.push_str(&format!(
                    "{:.2},{:.2},{:.2},{:.2},{}\n",
                    o + j,
                    o + 2.0 * j,
                    -o + j,
                    o * 0.5 + j,
                    name
                ));
            }
        }
        s
    }

    #[test]
    fn pca_projects_every_row_and_names_the_axes() {
        let proj = project(&sample(), &Options::default()).unwrap();
        assert_eq!(proj.n, 18);
        assert_eq!(proj.variables, 4);
        assert_eq!(proj.variable_names, vec!["f1", "f2", "f3", "f4"]);
        assert_eq!(proj.label_name, "species");
        assert_eq!(proj.categories, vec!["alpha", "beta", "gamma"]);
        assert!(proj.x_axis.starts_with("PC1 — "));
        assert!(proj.explained_variance.unwrap()[0] > 0.9);
        assert_eq!(proj.points[0].label, "alpha");
    }

    #[test]
    fn pca_separates_the_clusters() {
        let proj = project(&sample(), &Options::default()).unwrap();
        let centroid = |name: &str| {
            let pts: Vec<&Point> = proj.points.iter().filter(|p| p.label == name).collect();
            pts.iter().map(|p| p.x).sum::<f64>() / pts.len() as f64
        };
        let (a, b, c) = (centroid("alpha"), centroid("beta"), centroid("gamma"));
        // The three groups sit at clearly different places along PC1.
        assert!((a - b).abs() > 1.0 && (b - c).abs() > 1.0);
    }

    #[test]
    fn pca_is_deterministic() {
        let first = render(&sample(), &Options::default()).unwrap();
        let second = render(&sample(), &Options::default()).unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn tsne_is_deterministic_and_keeps_clusters_together() {
        let opts = Options {
            method: Method::Tsne,
            iterations: 300,
            ..Options::default()
        };
        let a = project(&sample(), &opts).unwrap();
        let b = project(&sample(), &opts).unwrap();
        assert_eq!(a.points, b.points, "no RNG — t-SNE must be reproducible");
        assert_eq!(a.perplexity_used, Some(17.0 / 3.0)); // clamped to (n-1)/3
        assert!(a.x_axis.starts_with("t-SNE"));
        assert!(a.explained_variance.is_none());

        // Same-species points end up closer to each other than to other species.
        let within: f64 = {
            let g: Vec<&Point> = a.points.iter().filter(|p| p.label == "alpha").collect();
            let cx = g.iter().map(|p| p.x).sum::<f64>() / g.len() as f64;
            let cy = g.iter().map(|p| p.y).sum::<f64>() / g.len() as f64;
            g.iter()
                .map(|p| ((p.x - cx).powi(2) + (p.y - cy).powi(2)).sqrt())
                .sum::<f64>()
                / g.len() as f64
        };
        let spread: f64 = {
            let cx = a.points.iter().map(|p| p.x).sum::<f64>() / a.n as f64;
            let cy = a.points.iter().map(|p| p.y).sum::<f64>() / a.n as f64;
            a.points
                .iter()
                .map(|p| ((p.x - cx).powi(2) + (p.y - cy).powi(2)).sqrt())
                .sum::<f64>()
                / a.n as f64
        };
        assert!(within < spread, "within={within} spread={spread}");
    }

    #[test]
    fn svg_has_one_circle_per_point_plus_the_legend() {
        let svg = render(&sample(), &Options::default()).unwrap();
        assert!(svg.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\""));
        assert!(svg.ends_with("</svg>"));
        // 18 data points + 3 legend swatches.
        assert_eq!(svg.matches("<circle").count(), 21);
        assert!(svg.contains(">alpha</text>"));
        assert!(svg.contains(">species</text>"));
        assert!(svg.contains("#2563eb") && svg.contains("#dc2626") && svg.contains("#16a34a"));
    }

    #[test]
    fn headerless_data_without_labels_plots_one_colour() {
        let data = "1 2 3\n2 4 6\n3 6 9\n4 8 12\n";
        let proj = project(data, &Options::default()).unwrap();
        assert_eq!(proj.n, 4);
        assert_eq!(proj.variable_names, vec!["v1", "v2", "v3"]);
        assert!(proj.categories.is_empty());
        let svg = render(data, &Options::default()).unwrap();
        assert_eq!(svg.matches("<circle").count(), 4);
        assert!(!svg.contains("#dc2626"));
    }

    #[test]
    fn label_column_can_be_named_or_indexed() {
        let by_name = project(
            &sample(),
            &Options {
                label_column: "species".into(),
                ..Options::default()
            },
        )
        .unwrap();
        let by_index = project(
            &sample(),
            &Options {
                label_column: "5".into(),
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(by_name.points, by_index.points);
        assert_eq!(by_index.label_name, "species");
    }

    #[test]
    fn a_numeric_column_can_be_used_as_the_group() {
        let data = "x,y,z,group\n1,2,3,1\n2,4,7,1\n8,1,2,2\n9,2,1,2\n";
        let proj = project(
            data,
            &Options {
                label_column: "group".into(),
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(proj.categories, vec!["1", "2"]);
        assert_eq!(proj.variables, 3);
    }

    #[test]
    fn csv_and_json_formats_carry_the_coordinates() {
        let csv = render(
            &sample(),
            &Options {
                format: Format::Csv,
                ..Options::default()
            },
        )
        .unwrap();
        assert!(csv.starts_with("index,label,pc1,pc2\n"));
        assert_eq!(csv.lines().count(), 19);
        assert!(csv.lines().nth(1).unwrap().starts_with("1,alpha,"));

        let json = render(
            &sample(),
            &Options {
                format: Format::Json,
                ..Options::default()
            },
        )
        .unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["method"], "pca");
        assert_eq!(v["points"].as_array().unwrap().len(), 18);
    }

    #[test]
    fn show_labels_draws_text_per_point() {
        let svg = render(
            &sample(),
            &Options {
                show_labels: true,
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(svg.matches(">alpha</text>").count(), 7); // 6 points + 1 legend entry
    }

    #[test]
    fn rejects_a_second_text_column() {
        let data = "name,note,x,y\na,hi,1,2\nb,ho,3,4\nc,he,5,6\n";
        let err = project(data, &Options::default()).unwrap_err();
        assert!(err.contains("is not numeric"), "{err}");
    }

    #[test]
    fn rejects_an_unknown_label_column() {
        let err = project(
            &sample(),
            &Options {
                label_column: "nope".into(),
                ..Options::default()
            },
        )
        .unwrap_err();
        assert!(err.contains("no column named 'nope'"), "{err}");
    }

    #[test]
    fn rejects_ragged_rows_and_too_few_observations() {
        let err = project("a,b,c\n1,2,3\n4,5\n", &Options::default()).unwrap_err();
        assert!(err.contains("columns"), "{err}");
        let err = project("1,2\n3,4\n", &Options::default()).unwrap_err();
        assert!(err.contains("at least 3 observations"), "{err}");
        let err = project("", &Options::default()).unwrap_err();
        assert!(err.contains("no data"), "{err}");
    }

    #[test]
    fn rejects_out_of_range_plot_settings() {
        let err = render(
            &sample(),
            &Options {
                width: 100,
                ..Options::default()
            },
        )
        .unwrap_err();
        assert!(err.contains("width must be"), "{err}");
        let err = render(
            &sample(),
            &Options {
                point_size: 99.0,
                ..Options::default()
            },
        )
        .unwrap_err();
        assert!(err.contains("point_size"), "{err}");
    }

    #[test]
    fn method_and_format_parse_leniently() {
        assert_eq!(Method::parse("PCA").unwrap(), Method::Pca);
        assert_eq!(Method::parse("t-SNE").unwrap(), Method::Tsne);
        assert_eq!(Method::parse("").unwrap(), Method::Pca);
        assert!(Method::parse("umap").unwrap_err().contains("unknown method"));
        assert_eq!(Format::parse("JSON").unwrap(), Format::Json);
        assert!(Format::parse("png").unwrap_err().contains("unknown format"));
    }

    #[test]
    fn escapes_markup_in_labels_and_titles() {
        let data = "x,y,g\n1,2,<a&b>\n3,4,<a&b>\n5,6,plain\n";
        let svg = render(
            data,
            &Options {
                title: "R&D <plot>".into(),
                ..Options::default()
            },
        )
        .unwrap();
        assert!(svg.contains("R&amp;D &lt;plot&gt;"));
        assert!(svg.contains(">&lt;a&amp;b&gt;</text>"));
        assert!(!svg.contains("<a&b>"));
    }

    /// The flat wrapper with every default spelled out — what chat/CLI/web send.
    fn run_defaults(data: &str) -> Result<String, String> {
        run(
            data, "pca", "", true, 30.0, 500.0, 200.0, false, 4.0, "", 720.0, 520.0, "svg",
        )
    }

    #[test]
    fn flat_wrapper_matches_the_options_api() {
        assert_eq!(
            run_defaults(&sample()).unwrap(),
            render(&sample(), &Options::default()).unwrap()
        );
        let csv = run(
            &sample(), "tsne", "species", false, 5.0, 60.0, 100.0, false, 6.0, "t", 400.0, 300.0,
            "csv",
        )
        .unwrap();
        assert!(csv.starts_with("index,label,tsne1,tsne2\n"), "{csv}");
        assert_eq!(csv.lines().count(), 19);
    }

    #[test]
    fn flat_wrapper_range_checks_the_numbers() {
        let bad = |what: &str, w: f64, h: f64, it: f64| {
            let e = run(
                &sample(), "pca", "", true, 30.0, it, 200.0, false, 4.0, "", w, h, "svg",
            )
            .unwrap_err();
            assert!(e.contains(what), "{e}");
        };
        bad("width", 120.0, 520.0, 500.0);
        bad("height", 720.0, 40.0, 500.0);
        bad("iterations", 720.0, 520.0, 10.0);
        bad("iterations must be a whole number", 720.0, 520.0, 500.5);
        let e = run(
            &sample(), "umap", "", true, 30.0, 500.0, 200.0, false, 4.0, "", 720.0, 520.0, "svg",
        )
        .unwrap_err();
        assert!(e.contains("unknown method"), "{e}");
    }
}
