//! random-projection-reducer core — reduce a wide numeric table to `k` columns
//! with a **Johnson–Lindenstrauss random projection**.
//!
//! The data matrix `X` (`n` rows × `d` columns) is multiplied by the transpose of
//! a randomly drawn `k × d` matrix `R`, giving `n × k` coordinates. Every
//! distribution `R` can be drawn from is normalised so that `E‖Rx‖² = ‖x‖²`,
//! which is what makes pairwise distances survive the reduction:
//!
//! * `gaussian`   — dense, entries `N(0, 1/k)`;
//! * `sparse`     — entries `±√(1/(p·k))` with probability `p/2` each, 0 otherwise,
//!                  where the default density `p = 1/√d`;
//! * `achlioptas` — the same family at the fixed density `p = 1/3`,
//!                  i.e. `√(3/k)·{−1, 0, +1}` with probabilities `1/6, 2/3, 1/6`;
//! * `rademacher` — dense `±√(1/k)` with probability `1/2` each.
//!
//! Randomness comes from a **portable integer stream** (SplitMix64 seeding
//! xoshiro256++), not from a platform RNG, so a given `seed` produces identical
//! numbers natively, in the CLI (wasm32-wasip1) and in the browser
//! (wasm32-unknown-unknown). Every reported number is rounded to 6 decimals.
//!
//! Besides the projection the tool measures how well it worked: it compares
//! pairwise row distances before and after and reports the mean, median and
//! maximum distortion, plus how many sampled pairs landed inside the requested
//! `±eps` — the check the Johnson–Lindenstrauss lemma is a statement about.

use serde::Serialize;

/// Maximum number of observations (rows) accepted.
pub const MAX_ROWS: usize = 2_000;
/// Maximum number of variables (columns) accepted.
pub const MAX_COLS: usize = 1_000;
/// Maximum number of cells (rows × columns) accepted.
pub const MAX_CELLS: usize = 200_000;
/// Maximum target dimensionality `k`.
pub const MAX_COMPONENTS: usize = 256;
/// Row count at or below which every pair is measured instead of sampled.
pub const ALL_PAIRS_ROWS: usize = 200;
/// Maximum number of row pairs measured for the distortion diagnostics.
pub const MAX_PAIRS: usize = 20_000;
/// How many projected rows the formatted text output prints before truncating.
const ROWS_TEXT_LIMIT: usize = 20;
/// Largest projection matrix (entries) embedded in the JSON output.
const JSON_MATRIX_LIMIT: usize = 20_000;
/// The eps values tabulated in the Johnson–Lindenstrauss guidance block.
const GUIDANCE_EPS: [f64; 4] = [0.5, 0.2, 0.1, 0.05];

// ---------------------------------------------------------------- RNG

/// xoshiro256++ seeded through SplitMix64 — a fixed integer stream, identical on
/// every backend (no platform RNG, no floating-point state).
struct Rng {
    s: [u64; 4],
}

impl Rng {
    fn new(seed: u64) -> Self {
        let mut z = seed;
        let mut next = || {
            z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut x = z;
            x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            x ^ (x >> 31)
        };
        Rng {
            s: [next(), next(), next(), next()],
        }
    }

    fn next_u64(&mut self) -> u64 {
        let result = self.s[0]
            .wrapping_add(self.s[3])
            .rotate_left(23)
            .wrapping_add(self.s[0]);
        let t = self.s[1] << 17;
        self.s[2] ^= self.s[0];
        self.s[3] ^= self.s[1];
        self.s[1] ^= self.s[2];
        self.s[0] ^= self.s[3];
        self.s[2] ^= t;
        self.s[3] = self.s[3].rotate_left(45);
        result
    }

    /// Uniform in `[0, 1)` with 53 bits of precision.
    fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / 9_007_199_254_740_992.0)
    }

    /// Uniform integer in `[0, n)` (n > 0), via Lemire's multiply-shift.
    fn next_below(&mut self, n: u64) -> u64 {
        ((self.next_u64() as u128 * n as u128) >> 64) as u64
    }

    /// Standard normal via the Box–Muller transform; the paired value is kept
    /// so two draws cost one transform.
    fn next_normal(&mut self, spare: &mut Option<f64>) -> f64 {
        if let Some(v) = spare.take() {
            return v;
        }
        // u1 must be strictly positive for ln().
        let u1 = 1.0 - self.next_f64();
        let u2 = self.next_f64();
        let r = (-2.0 * u1.ln()).sqrt();
        let theta = std::f64::consts::TAU * u2;
        *spare = Some(r * theta.sin());
        r * theta.cos()
    }
}

// ---------------------------------------------------------------- parameters

/// Which random matrix to draw.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    /// Dense Gaussian matrix, entries `N(0, 1/k)`.
    Gaussian,
    /// Sparse `±√(1/(p·k))` matrix; default density `p = 1/√d`.
    Sparse,
    /// Sparse matrix at the fixed Achlioptas density `p = 1/3`.
    Achlioptas,
    /// Dense `±√(1/k)` matrix (the ±1 "database-friendly" projection).
    Rademacher,
}

impl Method {
    /// Parse the `method` parameter (case-insensitive; `-`/`_` are ignored).
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().replace(['-', '_'], "").as_str() {
            "" | "gaussian" | "normal" => Ok(Method::Gaussian),
            "sparse" => Ok(Method::Sparse),
            "achlioptas" => Ok(Method::Achlioptas),
            "rademacher" | "pm1" | "sign" => Ok(Method::Rademacher),
            other => Err(format!(
                "unknown method '{other}' — use 'gaussian', 'sparse', 'achlioptas' or 'rademacher'"
            )),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Method::Gaussian => "gaussian",
            Method::Sparse => "sparse",
            Method::Achlioptas => "achlioptas",
            Method::Rademacher => "rademacher",
        }
    }

    fn blurb(self) -> &'static str {
        match self {
            Method::Gaussian => "dense matrix, entries drawn from N(0, 1/k)",
            Method::Sparse => "sparse ±sqrt(1/(density·k)) matrix, zeros elsewhere",
            Method::Achlioptas => "sparse sqrt(3/k)·{-1, 0, +1} matrix at density 1/3",
            Method::Rademacher => "dense ±sqrt(1/k) matrix, each sign equally likely",
        }
    }

    fn is_sparse_family(self) -> bool {
        matches!(self, Method::Sparse | Method::Achlioptas)
    }
}

/// Output representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// Formatted report: settings, distortion diagnostics, JL guidance, first rows.
    Text,
    /// Full structured result including every projected row.
    Json,
    /// The projected rows only, as `row,RP1,RP2,…`.
    Csv,
    /// The `k × d` projection matrix itself, as CSV.
    Matrix,
}

impl Format {
    /// Parse the `format` parameter (case-insensitive).
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "text" => Ok(Format::Text),
            "json" => Ok(Format::Json),
            "csv" => Ok(Format::Csv),
            "matrix" => Ok(Format::Matrix),
            other => Err(format!(
                "unknown format '{other}' — use 'text', 'json', 'csv' or 'matrix'"
            )),
        }
    }
}

/// How the target dimensionality was decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KSource {
    /// Derived from the Johnson–Lindenstrauss bound at `eps`.
    Auto,
    /// Given as a plain number.
    Explicit,
    /// Given as a percentage of the input column count.
    Percent,
}

// ---------------------------------------------------------------- output types

/// Distance-preservation diagnostics over sampled row pairs.
#[derive(Debug, Clone, Serialize)]
pub struct Distortion {
    /// Row pairs actually measured.
    pub pairs_measured: usize,
    /// Row pairs available in the data (`n·(n−1)/2`).
    pub pairs_total: usize,
    /// Pairs skipped because the two rows are identical (zero original distance).
    pub pairs_skipped: usize,
    /// Mean `|projected/original − 1|`.
    pub mean: f64,
    /// Median `|projected/original − 1|`.
    pub median: f64,
    /// Largest `|projected/original − 1|`.
    pub max: f64,
    /// Mean of `projected/original`.
    pub mean_ratio: f64,
    /// Pairs whose distortion is at most `eps`.
    pub within_eps: usize,
    /// `within_eps / pairs_measured`.
    pub within_eps_fraction: f64,
}

/// The full result of a random projection.
#[derive(Debug, Clone, Serialize)]
pub struct Projection {
    /// Observations in the input.
    pub rows: usize,
    /// Variables in the input.
    pub columns: usize,
    /// Target dimensionality actually used.
    pub components: usize,
    /// `"auto"`, `"explicit"` or `"percent"`.
    pub components_source: String,
    /// Matrix family used.
    pub method: String,
    /// Fraction of non-zero entries the matrix was drawn with.
    pub density: f64,
    /// Seed the matrix was drawn from.
    pub seed: u64,
    /// Distortion tolerance used for the JL bound and the "within ±eps" count.
    pub eps: f64,
    /// Column names, if the input carried a header row.
    pub column_names: Option<Vec<String>>,
    /// `⌈4·ln n / (eps²/2 − eps³/3)⌉` — the JL-safe dimensionality for `n` rows.
    pub jl_min_dim: usize,
    /// Whether that bound fits inside the input width.
    pub jl_reachable: bool,
    /// Distance-preservation diagnostics.
    pub distortion: Distortion,
    /// The projected data, `rows × components`.
    pub projected: Vec<Vec<f64>>,
    /// The projection matrix, `components × columns` (omitted when very large).
    pub matrix: Option<Vec<Vec<f64>>>,
    /// True when `matrix` was omitted for size; use `format = "matrix"` instead.
    pub matrix_omitted: bool,
    /// The exact matrix the projection used, at full precision — never serialized,
    /// so `format = "matrix"` reports what actually ran rather than a redraw.
    #[serde(skip)]
    pub raw_matrix: Vec<Vec<f64>>,
}

// ---------------------------------------------------------------- helpers

fn round6(v: f64) -> f64 {
    if !v.is_finite() {
        return v;
    }
    let r = (v * 1_000_000.0).round() / 1_000_000.0;
    if r == 0.0 {
        0.0
    } else {
        r
    }
}

/// Fixed 6-decimal rendering (`-0` normalised to `0.000000`).
fn f6(v: f64) -> String {
    format!("{:.6}", round6(v))
}

/// Fixed 4-decimal percentage rendering.
fn p4(v: f64) -> String {
    let r = (v * 10_000.0).round() / 10_000.0;
    format!("{:.4}%", if r == 0.0 { 0.0 } else { r })
}

fn split_row(line: &str) -> Vec<&str> {
    line.split(|c: char| c == ',' || c == '\t' || c == ';' || c == '|' || c.is_whitespace())
        .filter(|t| !t.is_empty())
        .collect()
}

/// `4·ln n / (eps²/2 − eps³/3)`, the Johnson–Lindenstrauss minimum dimension.
///
/// Truncated to a whole number, which is what the reference implementation does —
/// so the familiar published values (1e6 points at eps=0.5 → 663, at eps=0.1 →
/// 11841) come out unchanged here.
pub fn jl_min_dim(n_samples: usize, eps: f64) -> usize {
    let denom = eps * eps / 2.0 - eps * eps * eps / 3.0;
    let n = n_samples.max(2) as f64;
    (4.0 * n.ln() / denom).max(1.0) as usize
}

// ---------------------------------------------------------------- parsing

struct Table {
    header: Option<Vec<String>>,
    data: Vec<Vec<f64>>,
}

fn parse_table(data: &str) -> Result<Table, String> {
    let lines: Vec<&str> = data
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect();
    if lines.is_empty() {
        return Err("data is empty — paste a table with one observation per line, e.g. '1,2,3'"
            .to_string());
    }

    // A first row whose tokens are not all numeric is read as column names.
    let first = split_row(lines[0]);
    if first.len() < 2 {
        return Err(format!(
            "row 1 has {} value(s) — at least 2 columns are needed to reduce anything",
            first.len()
        ));
    }
    let header_row = !first.iter().all(|t| t.parse::<f64>().is_ok());
    let header = if header_row {
        Some(first.iter().map(|s| s.to_string()).collect::<Vec<_>>())
    } else {
        None
    };
    let body = if header_row { &lines[1..] } else { &lines[..] };
    if body.is_empty() {
        return Err(
            "data has a header row but no numbers under it — add at least 2 observation rows"
                .to_string(),
        );
    }

    let cols = first.len();
    if cols > MAX_COLS {
        return Err(format!(
            "{cols} columns exceeds the {MAX_COLS}-column limit"
        ));
    }
    if body.len() > MAX_ROWS {
        return Err(format!(
            "{} observation rows exceeds the {MAX_ROWS}-row limit",
            body.len()
        ));
    }
    if body.len() * cols > MAX_CELLS {
        return Err(format!(
            "{} rows × {cols} columns is {} cells, over the {MAX_CELLS}-cell limit",
            body.len(),
            body.len() * cols
        ));
    }
    if body.len() < 2 {
        return Err(format!(
            "only {} observation row(s) — at least 2 are needed to measure distance preservation",
            body.len()
        ));
    }

    let offset = if header_row { 2 } else { 1 };
    let mut out = Vec::with_capacity(body.len());
    for (i, line) in body.iter().enumerate() {
        let toks = split_row(line);
        if toks.len() != cols {
            return Err(format!(
                "row {} has {} values but row 1 has {cols} — every row must have the same number of columns",
                i + offset,
                toks.len()
            ));
        }
        let mut row = Vec::with_capacity(cols);
        for (j, tok) in toks.iter().enumerate() {
            match tok.parse::<f64>() {
                Ok(v) if v.is_finite() => row.push(v),
                _ => {
                    return Err(format!(
                        "row {}, column {}: '{}' is not a finite number",
                        i + offset,
                        j + 1,
                        tok
                    ))
                }
            }
        }
        out.push(row);
    }
    Ok(Table { header, data: out })
}

/// Resolve the `components` parameter into a target dimensionality.
fn resolve_k(components: &str, cols: usize, rows: usize, eps: f64) -> Result<(usize, KSource), String> {
    let raw = components.trim();
    if raw.is_empty() || raw.eq_ignore_ascii_case("auto") || raw == "0" {
        let want = jl_min_dim(rows, eps);
        return Ok((want.min(cols).min(MAX_COMPONENTS).max(1), KSource::Auto));
    }
    if let Some(pct) = raw.strip_suffix('%') {
        let p: f64 = pct.trim().parse().map_err(|_| {
            format!("components '{raw}' is not a valid percentage — try '25%'")
        })?;
        if !(p.is_finite() && p > 0.0) {
            return Err(format!(
                "components '{raw}' must be a percentage greater than 0, e.g. '25%'"
            ));
        }
        let k = ((cols as f64) * p / 100.0).round().max(1.0);
        if k > MAX_COMPONENTS as f64 {
            return Err(format!(
                "components '{raw}' of {cols} columns is {} dimensions, over the {MAX_COMPONENTS} limit",
                k as usize
            ));
        }
        return Ok((k as usize, KSource::Percent));
    }
    let k: f64 = raw
        .parse()
        .map_err(|_| format!("components '{raw}' is not a whole number, a percentage like '25%', or 'auto'"))?;
    if !k.is_finite() || k < 0.0 || k.fract() != 0.0 {
        return Err(format!(
            "components '{raw}' must be a whole number ≥ 0 (0 or 'auto' = derive it from eps)"
        ));
    }
    let k = k as usize;
    if k > MAX_COMPONENTS {
        return Err(format!(
            "components {k} exceeds the {MAX_COMPONENTS}-dimension limit"
        ));
    }
    Ok((k, KSource::Explicit))
}

/// Resolve the `density` parameter for the chosen method.
fn resolve_density(density: f64, method: Method, cols: usize) -> Result<f64, String> {
    if !density.is_finite() || density < 0.0 || density > 1.0 {
        return Err(format!(
            "density {density} must be between 0 and 1 (0 = the default density for the method)"
        ));
    }
    if density > 0.0 && !method.is_sparse_family() {
        return Err(format!(
            "density applies to method 'sparse' and 'achlioptas'; method '{}' has no zero entries — use method='sparse' to control sparsity",
            method.label()
        ));
    }
    Ok(match method {
        Method::Gaussian | Method::Rademacher => 1.0,
        Method::Achlioptas if density == 0.0 => 1.0 / 3.0,
        Method::Sparse if density == 0.0 => (1.0 / (cols as f64).sqrt()).clamp(1e-6, 1.0),
        _ => density,
    })
}

// ---------------------------------------------------------------- projection

fn build_matrix(method: Method, density: f64, k: usize, d: usize, seed: u64) -> Vec<Vec<f64>> {
    let mut rng = Rng::new(seed);
    let mut spare: Option<f64> = None;
    let inv_sqrt_k = 1.0 / (k as f64).sqrt();
    let sparse_value = (1.0 / (density * k as f64)).sqrt();
    let mut m = Vec::with_capacity(k);
    for _ in 0..k {
        let mut row = Vec::with_capacity(d);
        for _ in 0..d {
            let v = match method {
                Method::Gaussian => rng.next_normal(&mut spare) * inv_sqrt_k,
                Method::Rademacher => {
                    if rng.next_u64() & 1 == 0 {
                        -inv_sqrt_k
                    } else {
                        inv_sqrt_k
                    }
                }
                Method::Sparse | Method::Achlioptas => {
                    let u = rng.next_f64();
                    if u < density / 2.0 {
                        -sparse_value
                    } else if u < density {
                        sparse_value
                    } else {
                        0.0
                    }
                }
            };
            row.push(v);
        }
        m.push(row);
    }
    m
}

fn distance(a: &[f64], b: &[f64]) -> f64 {
    let mut s = 0.0;
    for i in 0..a.len() {
        let dv = a[i] - b[i];
        s += dv * dv;
    }
    s.sqrt()
}

fn measure(
    data: &[Vec<f64>],
    projected: &[Vec<f64>],
    eps: f64,
    seed: u64,
) -> (Distortion, usize) {
    let n = data.len();
    let pairs_total = n * (n - 1) / 2;
    let mut pairs: Vec<(usize, usize)> = Vec::new();
    if n <= ALL_PAIRS_ROWS {
        for i in 0..n {
            for j in (i + 1)..n {
                pairs.push((i, j));
            }
        }
    } else {
        // Deterministic sample from a stream independent of the matrix draw.
        let mut rng = Rng::new(seed ^ 0xA5A5_5A5A_C3C3_3C3C);
        while pairs.len() < MAX_PAIRS {
            let i = rng.next_below(n as u64) as usize;
            let j = rng.next_below(n as u64) as usize;
            if i != j {
                pairs.push((i.min(j), i.max(j)));
            }
        }
    }

    let mut ratios: Vec<f64> = Vec::with_capacity(pairs.len());
    let mut skipped = 0usize;
    for (i, j) in &pairs {
        let d0 = distance(&data[*i], &data[*j]);
        if d0 == 0.0 {
            skipped += 1;
            continue;
        }
        ratios.push(distance(&projected[*i], &projected[*j]) / d0);
    }

    let measured = ratios.len();
    if measured == 0 {
        return (
            Distortion {
                pairs_measured: 0,
                pairs_total,
                pairs_skipped: skipped,
                mean: 0.0,
                median: 0.0,
                max: 0.0,
                mean_ratio: 0.0,
                within_eps: 0,
                within_eps_fraction: 0.0,
            },
            pairs.len(),
        );
    }

    let mut dist: Vec<f64> = ratios.iter().map(|r| (r - 1.0).abs()).collect();
    let mean = dist.iter().sum::<f64>() / measured as f64;
    let mean_ratio = ratios.iter().sum::<f64>() / measured as f64;
    let max = dist.iter().cloned().fold(0.0_f64, f64::max);
    let within = dist.iter().filter(|v| **v <= eps).count();
    dist.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = if measured % 2 == 1 {
        dist[measured / 2]
    } else {
        (dist[measured / 2 - 1] + dist[measured / 2]) / 2.0
    };

    (
        Distortion {
            pairs_measured: measured,
            pairs_total,
            pairs_skipped: skipped,
            mean: round6(mean),
            median: round6(median),
            max: round6(max),
            mean_ratio: round6(mean_ratio),
            within_eps: within,
            within_eps_fraction: round6(within as f64 / measured as f64),
        },
        pairs.len(),
    )
}

/// Run a random projection and return the full structured result.
#[allow(clippy::too_many_arguments)]
pub fn project(
    data: &str,
    components: &str,
    method: &str,
    density: f64,
    eps: f64,
    seed: f64,
) -> Result<Projection, String> {
    if !eps.is_finite() || !(0.01..=0.99).contains(&eps) {
        return Err(format!(
            "eps {eps} must be between 0.01 and 0.99 — it is the distance-distortion tolerance, e.g. 0.1 for ±10%"
        ));
    }
    if !seed.is_finite() || seed < 0.0 || seed.fract() != 0.0 || seed > 4_294_967_295.0 {
        return Err(format!(
            "seed {seed} must be a whole number between 0 and 4294967295"
        ));
    }
    let seed = seed as u64;
    let method = Method::parse(method)?;
    let table = parse_table(data)?;
    let n = table.data.len();
    let d = table.data[0].len();
    let (k, k_source) = resolve_k(components, d, n, eps)?;
    if k == 0 {
        return Err("components resolved to 0 dimensions — ask for at least 1".to_string());
    }
    let density = resolve_density(density, method, d)?;

    let matrix = build_matrix(method, density, k, d, seed);
    let mut projected = Vec::with_capacity(n);
    for row in &table.data {
        let mut out = Vec::with_capacity(k);
        for comp in &matrix {
            let mut s = 0.0;
            for j in 0..d {
                s += row[j] * comp[j];
            }
            out.push(round6(s));
        }
        projected.push(out);
    }

    let (distortion, _) = measure(&table.data, &projected, eps, seed);
    let jl = jl_min_dim(n, eps);
    let keep_matrix = k * d <= JSON_MATRIX_LIMIT;

    Ok(Projection {
        rows: n,
        columns: d,
        components: k,
        components_source: match k_source {
            KSource::Auto => "auto".into(),
            KSource::Explicit => "explicit".into(),
            KSource::Percent => "percent".into(),
        },
        method: method.label().to_string(),
        density: round6(density),
        seed,
        eps,
        column_names: table.header.clone(),
        jl_min_dim: jl,
        jl_reachable: jl <= d,
        distortion,
        projected,
        matrix: if keep_matrix {
            Some(
                matrix
                    .iter()
                    .map(|r| r.iter().map(|v| round6(*v)).collect())
                    .collect(),
            )
        } else {
            None
        },
        matrix_omitted: !keep_matrix,
        raw_matrix: matrix,
    })
}

// ---------------------------------------------------------------- rendering

fn render_text(p: &Projection) -> String {
    let mut s = String::new();
    s.push_str(&format!(
        "Random projection: {} rows × {} columns → {} dimensions ({} of the input width)\n",
        p.rows,
        p.columns,
        p.components,
        p4(p.components as f64 * 100.0 / p.columns as f64)
    ));
    s.push_str(&format!(
        "  method        {} — {}\n",
        p.method,
        match p.method.as_str() {
            "gaussian" => Method::Gaussian.blurb(),
            "sparse" => Method::Sparse.blurb(),
            "achlioptas" => Method::Achlioptas.blurb(),
            _ => Method::Rademacher.blurb(),
        }
    ));
    s.push_str(&format!(
        "  target dims   {} ({})\n",
        p.components,
        match p.components_source.as_str() {
            "auto" => format!(
                "auto, from the Johnson–Lindenstrauss bound at eps={}",
                f6(p.eps)
            ),
            "percent" => "set as a percentage of the input width".to_string(),
            _ => "set explicitly".to_string(),
        }
    ));
    s.push_str(&format!(
        "  density       {} non-zero entries\n",
        p4(p.density * 100.0)
    ));
    s.push_str(&format!("  seed          {}\n", p.seed));
    if let Some(names) = &p.column_names {
        s.push_str(&format!("  input columns {}\n", names.join(", ")));
    }

    s.push_str(&format!(
        "\nDistance preservation ({} of {} row pairs measured):\n",
        p.distortion.pairs_measured, p.distortion.pairs_total
    ));
    s.push_str(&format!(
        "  mean distortion     {}\n",
        p4(p.distortion.mean * 100.0)
    ));
    s.push_str(&format!(
        "  median distortion   {}\n",
        p4(p.distortion.median * 100.0)
    ));
    s.push_str(&format!(
        "  max distortion      {}\n",
        p4(p.distortion.max * 100.0)
    ));
    s.push_str(&format!(
        "  mean ratio          {}  (projected ÷ original distance)\n",
        f6(p.distortion.mean_ratio)
    ));
    s.push_str(&format!(
        "  within ±{}  {} of {} pairs ({})\n",
        p4(p.eps * 100.0),
        p.distortion.within_eps,
        p.distortion.pairs_measured,
        p4(p.distortion.within_eps_fraction * 100.0)
    ));
    if p.distortion.pairs_skipped > 0 {
        s.push_str(&format!(
            "  skipped             {} pair(s) of identical rows (zero distance)\n",
            p.distortion.pairs_skipped
        ));
    }

    s.push_str(&format!(
        "\nJohnson–Lindenstrauss guidance for {} rows:\n",
        p.rows
    ));
    s.push_str("  eps     min k\n");
    for e in GUIDANCE_EPS {
        s.push_str(&format!("  {:<6}  {}\n", format!("{:.2}", e), jl_min_dim(p.rows, e)));
    }
    if p.jl_reachable {
        s.push_str(&format!(
            "  A guaranteed ±{} embedding of {} points needs k ≥ {}, which fits inside the {} input columns.\n",
            p4(p.eps * 100.0),
            p.rows,
            p.jl_min_dim,
            p.columns
        ));
    } else {
        s.push_str(&format!(
            "  A guaranteed ±{} embedding of {} points needs k ≥ {}, more than the {} input columns,\n  so the bound is out of reach at this size — the measured distortion above is what actually happened.\n",
            p4(p.eps * 100.0),
            p.rows,
            p.jl_min_dim,
            p.columns
        ));
    }

    let shown = p.projected.len().min(ROWS_TEXT_LIMIT);
    s.push_str(&format!(
        "\nProjected data (first {} of {} rows):\n",
        shown, p.rows
    ));
    let mut head = String::from("  row");
    for i in 1..=p.components {
        head.push_str(&format!("  {:>12}", format!("RP{i}")));
    }
    s.push_str(&head);
    s.push('\n');
    for (i, row) in p.projected.iter().take(shown).enumerate() {
        let mut line = format!("  {:>3}", i + 1);
        for v in row {
            line.push_str(&format!("  {:>12}", f6(*v)));
        }
        s.push_str(&line);
        s.push('\n');
    }
    if p.rows > shown {
        s.push_str(&format!(
            "  … {} more row(s) — use format='csv' or 'json' for all of them\n",
            p.rows - shown
        ));
    }
    s.trim_end().to_string()
}

fn render_csv(p: &Projection) -> String {
    let mut s = String::from("row");
    for i in 1..=p.components {
        s.push_str(&format!(",RP{i}"));
    }
    s.push('\n');
    for (i, row) in p.projected.iter().enumerate() {
        s.push_str(&(i + 1).to_string());
        for v in row {
            s.push(',');
            s.push_str(&f6(*v));
        }
        s.push('\n');
    }
    s.trim_end().to_string()
}

fn render_matrix(p: &Projection) -> String {
    let matrix = &p.raw_matrix;
    let mut s = String::from("component");
    match &p.column_names {
        Some(names) => {
            for n in names {
                s.push(',');
                s.push_str(&n.replace(',', " "));
            }
        }
        None => {
            for j in 1..=p.columns {
                s.push_str(&format!(",v{j}"));
            }
        }
    }
    s.push('\n');
    for (i, row) in matrix.iter().enumerate() {
        s.push_str(&format!("RP{}", i + 1));
        for v in row {
            s.push(',');
            s.push_str(&f6(*v));
        }
        s.push('\n');
    }
    s.trim_end().to_string()
}

/// Entry point shared by the chat block, the CLI and the browser page.
pub fn run(
    data: &str,
    components: &str,
    method: &str,
    density: f64,
    eps: f64,
    seed: f64,
    format: &str,
) -> Result<String, String> {
    let fmt = Format::parse(format)?;
    let p = project(data, components, method, density, eps, seed)?;
    Ok(match fmt {
        Format::Text => render_text(&p),
        Format::Csv => render_csv(&p),
        Format::Json => serde_json::to_string_pretty(&p).map_err(|e| e.to_string())?,
        // Always available at full precision, even when the matrix was too large
        // to embed in the JSON result.
        Format::Matrix => render_matrix(&p),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const WIDE: &str = "a,b,c,d,e,f,g,h\n1,2,3,4,5,6,7,8\n8,7,6,5,4,3,2,1\n2,4,6,8,10,12,14,16\n0,1,0,1,0,1,0,1\n5,5,5,5,5,5,5,5\n9,1,8,2,7,3,6,4";

    #[test]
    fn projects_to_the_requested_dimensions() {
        let out = run(WIDE, "3", "gaussian", 0.0, 0.1, 42.0, "csv").unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[0], "row,RP1,RP2,RP3");
        assert_eq!(lines.len(), 7, "header + 6 rows");
        for line in &lines[1..] {
            assert_eq!(line.split(',').count(), 4, "row id + 3 coordinates");
        }
    }

    #[test]
    fn header_row_is_detected_and_reported() {
        let out = run(WIDE, "2", "sparse", 0.0, 0.1, 7.0, "text").unwrap();
        assert!(out.contains("Random projection: 6 rows × 8 columns → 2 dimensions"));
        assert!(out.contains("input columns a, b, c, d, e, f, g, h"));
        assert!(out.contains("Distance preservation (15 of 15 row pairs measured)"));
    }

    #[test]
    fn same_seed_is_reproducible_and_different_seeds_differ() {
        let a = run(WIDE, "4", "gaussian", 0.0, 0.1, 1.0, "csv").unwrap();
        let b = run(WIDE, "4", "gaussian", 0.0, 0.1, 1.0, "csv").unwrap();
        let c = run(WIDE, "4", "gaussian", 0.0, 0.1, 2.0, "csv").unwrap();
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn rademacher_matrix_entries_are_plus_or_minus_one_over_sqrt_k() {
        let out = run(WIDE, "4", "rademacher", 0.0, 0.1, 3.0, "matrix").unwrap();
        let expect = format!("{:.6}", 0.5_f64); // 1/sqrt(4)
        for line in out.lines().skip(1) {
            for cell in line.split(',').skip(1) {
                assert!(
                    cell == expect || cell == format!("-{expect}"),
                    "unexpected entry {cell}"
                );
            }
        }
    }

    #[test]
    fn achlioptas_entries_are_the_three_allowed_values() {
        let out = run(WIDE, "3", "achlioptas", 0.0, 0.1, 5.0, "matrix").unwrap();
        let v = format!("{:.6}", (3.0_f64 / 3.0).sqrt()); // sqrt(1/(1/3 * 3)) = 1
        let mut zeros = 0;
        for line in out.lines().skip(1) {
            for cell in line.split(',').skip(1) {
                if cell == "0.000000" {
                    zeros += 1;
                } else {
                    assert!(cell == v || cell == format!("-{v}"), "unexpected entry {cell}");
                }
            }
        }
        assert!(zeros > 0, "an Achlioptas matrix at density 1/3 must contain zeros");
    }

    #[test]
    fn distances_are_approximately_preserved_on_random_data() {
        // 60 rows × 200 columns projected to 64 dimensions: mean distortion should
        // be small even though the JL bound for 60 points is far larger.
        let mut rng = Rng::new(11);
        let mut rows = Vec::new();
        for _ in 0..60 {
            let cells: Vec<String> = (0..200).map(|_| format!("{:.4}", rng.next_f64())).collect();
            rows.push(cells.join(","));
        }
        let data = rows.join("\n");
        let p = project(&data, "64", "gaussian", 0.0, 0.1, 42.0).unwrap();
        assert_eq!(p.rows, 60);
        assert_eq!(p.columns, 200);
        assert_eq!(p.components, 64);
        assert!(
            p.distortion.mean < 0.1,
            "mean distortion {} should be under 10%",
            p.distortion.mean
        );
        assert!(
            p.distortion.mean_ratio > 0.9 && p.distortion.mean_ratio < 1.1,
            "mean ratio {} should sit near 1",
            p.distortion.mean_ratio
        );
    }

    #[test]
    fn auto_components_follow_the_jl_bound_clamped_to_the_input_width() {
        let p = project(WIDE, "auto", "gaussian", 0.0, 0.1, 42.0).unwrap();
        assert_eq!(p.components_source, "auto");
        assert_eq!(p.jl_min_dim, jl_min_dim(6, 0.1));
        assert!(!p.jl_reachable);
        assert_eq!(p.components, 8, "clamped to the 8 input columns");
    }

    #[test]
    fn jl_min_dim_matches_the_published_values() {
        assert_eq!(jl_min_dim(1_000_000, 0.5), 663);
        assert_eq!(jl_min_dim(1_000_000, 0.1), 11_841);
        assert_eq!(jl_min_dim(10_000, 0.1), 7_894);
    }

    #[test]
    fn percentage_components_use_the_input_width() {
        let p = project(WIDE, "25%", "gaussian", 0.0, 0.1, 42.0).unwrap();
        assert_eq!(p.components, 2);
        assert_eq!(p.components_source, "percent");
    }

    #[test]
    fn sparse_default_density_is_one_over_sqrt_d() {
        let p = project(WIDE, "3", "sparse", 0.0, 0.1, 42.0).unwrap();
        assert_eq!(p.density, round6(1.0 / (8.0_f64).sqrt()));
    }

    #[test]
    fn json_carries_every_row_and_the_matrix() {
        let out = run(WIDE, "3", "gaussian", 0.0, 0.1, 42.0, "json").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["rows"], 6);
        assert_eq!(v["components"], 3);
        assert_eq!(v["projected"].as_array().unwrap().len(), 6);
        assert_eq!(v["matrix"].as_array().unwrap().len(), 3);
        assert_eq!(v["matrix"][0].as_array().unwrap().len(), 8);
        assert_eq!(v["matrix_omitted"], false);
    }

    #[test]
    fn ragged_rows_are_rejected_with_the_row_number() {
        let err = run("1,2,3\n4,5\n7,8,9", "2", "gaussian", 0.0, 0.1, 42.0, "text").unwrap_err();
        assert_eq!(
            err,
            "row 2 has 2 values but row 1 has 3 — every row must have the same number of columns"
        );
    }

    #[test]
    fn non_numeric_cells_are_rejected_with_a_location() {
        let err = run("1,2,3\n4,five,6", "2", "gaussian", 0.0, 0.1, 42.0, "text").unwrap_err();
        assert_eq!(err, "row 2, column 2: 'five' is not a finite number");
    }

    #[test]
    fn empty_data_is_rejected() {
        let err = run("   \n\n", "2", "gaussian", 0.0, 0.1, 42.0, "text").unwrap_err();
        assert!(err.starts_with("data is empty"), "{err}");
    }

    #[test]
    fn a_single_column_is_rejected() {
        let err = run("1\n2\n3", "1", "gaussian", 0.0, 0.1, 42.0, "text").unwrap_err();
        assert!(err.contains("at least 2 columns"), "{err}");
    }

    #[test]
    fn a_single_row_is_rejected() {
        let err = run("1,2,3", "2", "gaussian", 0.0, 0.1, 42.0, "text").unwrap_err();
        assert!(err.contains("at least 2 are needed"), "{err}");
    }

    #[test]
    fn unknown_method_and_format_are_rejected() {
        let err = run(WIDE, "2", "umap", 0.0, 0.1, 42.0, "text").unwrap_err();
        assert!(err.starts_with("unknown method 'umap'"), "{err}");
        let err = run(WIDE, "2", "gaussian", 0.0, 0.1, 42.0, "xml").unwrap_err();
        assert!(err.starts_with("unknown format 'xml'"), "{err}");
    }

    #[test]
    fn density_is_rejected_for_dense_methods() {
        let err = run(WIDE, "2", "gaussian", 0.25, 0.1, 42.0, "text").unwrap_err();
        assert!(err.contains("method='sparse'"), "{err}");
    }

    #[test]
    fn out_of_range_eps_seed_and_components_are_rejected() {
        assert!(run(WIDE, "2", "gaussian", 0.0, 0.0, 42.0, "text")
            .unwrap_err()
            .contains("between 0.01 and 0.99"));
        assert!(run(WIDE, "2", "gaussian", 0.0, 0.1, -1.0, "text")
            .unwrap_err()
            .contains("between 0 and 4294967295"));
        assert!(run(WIDE, "999", "gaussian", 0.0, 0.1, 42.0, "text")
            .unwrap_err()
            .contains("exceeds the 256-dimension limit"));
        assert!(run(WIDE, "two", "gaussian", 0.0, 0.1, 42.0, "text")
            .unwrap_err()
            .contains("not a whole number"));
    }

    #[test]
    fn identical_rows_are_reported_as_skipped_pairs() {
        let data = "1,2,3,4\n1,2,3,4\n9,8,7,6";
        let out = run(data, "2", "gaussian", 0.0, 0.1, 42.0, "text").unwrap();
        assert!(out.contains("skipped             1 pair(s) of identical rows"), "{out}");
    }

    #[test]
    fn the_cell_cap_is_enforced() {
        let row = vec!["1"; 500].join(",");
        let data = std::iter::repeat(row).take(401).collect::<Vec<_>>().join("\n");
        let err = run(&data, "2", "gaussian", 0.0, 0.1, 42.0, "text").unwrap_err();
        assert!(err.contains("over the 200000-cell limit"), "{err}");
    }
}
