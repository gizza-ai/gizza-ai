//! isolation-forest-anomaly core — pure compute, shared by the chat skill block
//! and the web page. No wafer/wasm-bindgen deps, no external crates.
//!
//! Builds an **Isolation Forest** over the numeric columns of a pasted CSV/TSV
//! table and returns a per-row anomaly score.
//!
//! The idea (Liu, Ting & Zhou, 2008): an anomaly is *easy to isolate*. Each tree
//! is grown on a small random subsample by repeatedly picking a random feature
//! and a random split value between that feature's current min and max, until
//! every point sits alone or a height limit is reached. Points that end up in a
//! shallow leaf were separated from the rest after only a few random cuts, which
//! is exactly what "unusual" means in this model.
//!
//! Scoring follows the paper (and the scikit-learn reference implementation):
//!
//! ```text
//! s(x) = 2 ^ ( -E(h(x)) / c(psi) )
//! c(n) = 2 * (ln(n - 1) + EULER) - 2 * (n - 1) / n
//! ```
//!
//! where `E(h(x))` is the row's average path length across the forest and
//! `c(psi)` is the average unsuccessful-search path length of a binary search
//! tree holding `psi` (the subsample size) nodes. Scores live in `(0, 1)`:
//! **higher = more anomalous**, `0.5` is the classic "typical point" cut-off,
//! which is what `contamination = auto` uses (the same place scikit-learn's
//! `auto` offset of -0.5 lands after the sign flip).
//!
//! Two variants are supported:
//!
//! - `standard` — axis-parallel cuts, the original algorithm. Splits are drawn
//!   between a feature's own min and max inside each node, so the result is
//!   invariant to per-column rescaling; there is deliberately no `scaling`
//!   parameter because it could not change anything.
//! - `extended` — randomly-oriented hyperplane cuts (Hariri, Kind & Brunner,
//!   2018), which removes the rectangular banding the original leaves in the
//!   score field for correlated columns. Hyperplanes mix columns, so this mode
//!   standardizes features internally before fitting.
//!
//! Everything is driven by a seeded xorshift RNG, so the same input plus the
//! same `seed` always produces byte-identical output — required by the page's
//! recompute-on-input model and by the tests.

/// Maximum number of data rows (header excluded) accepted in one run.
pub const MAX_ROWS: usize = 20_000;
/// Maximum number of columns in the pasted table.
pub const MAX_COLS: usize = 200;
/// Maximum number of trees in the forest.
pub const MAX_TREES: u32 = 1_000;

const EULER: f64 = 0.577_215_664_901_532_9;

/// Options resolved from the tool params.
pub struct Options {
    /// Comma-separated feature columns: header names or 1-based indices.
    /// Blank = every fully numeric column.
    pub features: String,
    /// "standard" (axis-parallel cuts) or "extended" (hyperplane cuts).
    pub method: String,
    /// Number of isolation trees in the forest.
    pub trees: u32,
    /// Subsample size per tree: "auto" (min(256, rows)), an integer count, or a
    /// fraction written with a decimal point or a percent sign.
    pub sample_size: String,
    /// Features drawn per tree: a fraction of the selected columns when <= 1,
    /// otherwise an absolute count.
    pub max_features: f64,
    /// Draw each tree's subsample with replacement.
    pub bootstrap: bool,
    /// Expected outlier rate: "auto" (score cut-off 0.5), a fraction such as
    /// `0.05`, or a percentage such as `5%`.
    pub contamination: String,
    /// Explicit score cut-off in (0, 1). 0 = derive it from `contamination`.
    pub threshold: f64,
    /// Non-numeric / blank cell policy: drop | median | mean | zero | error.
    pub missing: String,
    /// Row order in the output: "input" or "score" (most anomalous first).
    pub sort: String,
    /// Keep only the first N listed rows. 0 = all of them.
    pub top: u32,
    /// List only the rows flagged as anomalies.
    pub only_anomalies: bool,
    /// Whether the first row holds column names: auto | yes | no.
    pub header: String,
    /// Column delimiter: auto | comma | tab | semicolon | pipe | space, or a
    /// single character.
    pub delimiter: String,
    /// Decimal places for scores and path lengths.
    pub decimals: u32,
    /// Seed for the subsampling, the split features and the split values.
    pub seed: u64,
    /// Output format: text | json | csv.
    pub format: String,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            features: String::new(),
            method: "standard".into(),
            trees: 100,
            sample_size: "auto".into(),
            max_features: 1.0,
            bootstrap: false,
            contamination: "auto".into(),
            threshold: 0.0,
            missing: "drop".into(),
            sort: "input".into(),
            top: 0,
            only_anomalies: false,
            header: "auto".into(),
            delimiter: "auto".into(),
            decimals: 4,
            seed: 42,
            format: "text".into(),
        }
    }
}

// -------------------------------------------------------------------- rng ---

/// Deterministic xorshift64* generator. Seeded from `Options::seed` so a run is
/// exactly reproducible; a fixed odd constant keeps seed 0 usable.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Rng(seed ^ 0x9E37_79B9_7F4A_7C15)
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Uniform in `[0, 1)`.
    fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Uniform integer in `[0, n)`.
    fn below(&mut self, n: usize) -> usize {
        if n <= 1 {
            0
        } else {
            (self.next_u64() % n as u64) as usize
        }
    }

    /// Standard normal via Box-Muller (only used by the extended variant).
    fn normal(&mut self) -> f64 {
        let u1 = self.unit().max(1e-12);
        let u2 = self.unit();
        (-2.0 * u1.ln()).sqrt() * (core::f64::consts::TAU * u2).cos()
    }
}

// ---------------------------------------------------------------- parsing ---

fn is_missing(tok: &str) -> bool {
    matches!(
        tok.trim().to_ascii_lowercase().as_str(),
        "" | "na" | "n/a" | "nan" | "null" | "none" | "-" | "?" | "."
    )
}

fn parse_num(tok: &str) -> Option<f64> {
    let t = tok.trim();
    if t.is_empty() {
        return None;
    }
    t.parse::<f64>().ok().filter(|v| v.is_finite())
}

/// Split one line on `delim`, honouring `"` quoting (doubled `""` = a literal
/// quote). `None` means "any run of whitespace".
fn split_row(line: &str, delim: Option<char>) -> Vec<String> {
    let Some(d) = delim else {
        return line.split_whitespace().map(|s| s.to_string()).collect();
    };
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if quoted {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    cur.push('"');
                    chars.next();
                } else {
                    quoted = false;
                }
            } else {
                cur.push(c);
            }
        } else if c == '"' && cur.trim().is_empty() {
            cur.clear();
            quoted = true;
        } else if c == d {
            out.push(cur.trim().to_string());
            cur = String::new();
        } else {
            cur.push(c);
        }
    }
    out.push(cur.trim().to_string());
    out
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

fn resolve_delim(spec: &str, first_line: &str) -> Result<Option<char>, String> {
    match spec.trim().to_ascii_lowercase().as_str() {
        "" | "auto" => Ok(detect_delim(first_line)),
        "comma" | "," => Ok(Some(',')),
        "tab" | "\\t" => Ok(Some('\t')),
        "semicolon" | ";" => Ok(Some(';')),
        "pipe" | "|" => Ok(Some('|')),
        "space" | "whitespace" => Ok(None),
        other => {
            let mut it = other.chars();
            match (it.next(), it.next()) {
                (Some(c), None) => Ok(Some(c)),
                _ => Err(format!(
                    "delimiter must be auto, comma, tab, semicolon, pipe, space, or a single character (got '{other}')"
                )),
            }
        }
    }
}

struct Table {
    names: Vec<String>,
    rows: Vec<Vec<String>>,
    had_header: bool,
}

fn parse_table(data: &str, opts: &Options) -> Result<Table, String> {
    let lines: Vec<&str> = data
        .lines()
        .map(|l| l.trim_end_matches('\r'))
        .filter(|l| !l.trim().is_empty())
        .collect();
    let Some(first) = lines.first() else {
        return Err(
            "no data: paste a table with one row per observation, e.g. 'temp,pressure' then '20,101'"
                .into(),
        );
    };
    let delim = resolve_delim(&opts.delimiter, first)?;
    let mut rows: Vec<Vec<String>> = lines.iter().map(|l| split_row(l, delim)).collect();

    let ncol = rows[0].len();
    if ncol == 0 {
        return Err("the first row has no columns".into());
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

    let had_header = match opts.header.trim().to_ascii_lowercase().as_str() {
        "yes" | "true" => true,
        "no" | "false" => false,
        "" | "auto" => {
            rows.len() > 1
                && rows[0]
                    .iter()
                    .any(|t| !is_missing(t) && parse_num(t).is_none())
        }
        other => {
            return Err(format!(
                "header must be 'auto', 'yes' or 'no' (got '{other}')"
            ))
        }
    };

    let names: Vec<String> = if had_header {
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
            "too many rows: {} (max {MAX_ROWS}). Sample the table first, or run the tool on chunks.",
            rows.len()
        ));
    }
    Ok(Table {
        names,
        rows,
        had_header,
    })
}

/// Resolve the `features` selector into column indices.
fn resolve_features(spec: &str, t: &Table) -> Result<Vec<usize>, String> {
    let spec = spec.trim();
    if spec.is_empty() {
        // Every column whose non-missing cells are all numeric.
        let picked: Vec<usize> = (0..t.names.len())
            .filter(|&c| {
                let mut seen = 0usize;
                for r in &t.rows {
                    if is_missing(&r[c]) {
                        continue;
                    }
                    if parse_num(&r[c]).is_none() {
                        return false;
                    }
                    seen += 1;
                }
                seen > 0
            })
            .collect();
        if picked.is_empty() {
            return Err(
                "no numeric columns found — an isolation forest needs at least one numeric feature column. Name the columns explicitly with 'features' if they carry units or thousands separators."
                    .into(),
            );
        }
        return Ok(picked);
    }

    let mut out = Vec::new();
    for raw in spec.split(',') {
        let tok = raw.trim();
        if tok.is_empty() {
            continue;
        }
        let idx = if let Ok(n) = tok.parse::<usize>() {
            if n == 0 || n > t.names.len() {
                return Err(format!(
                    "feature column index {n} is out of range 1..{}",
                    t.names.len()
                ));
            }
            n - 1
        } else {
            match t.names.iter().position(|n| n.eq_ignore_ascii_case(tok)) {
                Some(i) => i,
                None => {
                    return Err(format!(
                        "no column named '{tok}' — available columns: {}",
                        t.names.join(", ")
                    ))
                }
            }
        };
        if !out.contains(&idx) {
            out.push(idx);
        }
    }
    if out.is_empty() {
        return Err("features listed no usable columns".into());
    }
    Ok(out)
}

// ---------------------------------------------------------------- forest ---

/// Average unsuccessful-search path length of a binary search tree of `n` nodes.
fn c_factor(n: usize) -> f64 {
    if n <= 1 {
        0.0
    } else if n == 2 {
        1.0
    } else {
        let nf = n as f64;
        2.0 * ((nf - 1.0).ln() + EULER) - 2.0 * (nf - 1.0) / nf
    }
}

enum Node {
    Leaf {
        size: usize,
    },
    Axis {
        feature: usize,
        value: f64,
        left: usize,
        right: usize,
    },
    Hyper {
        normal: Vec<f64>,
        offset: f64,
        left: usize,
        right: usize,
    },
}

struct Tree {
    nodes: Vec<Node>,
    /// Column indices (into the feature matrix) this tree may split on.
    feats: Vec<usize>,
}

impl Tree {
    fn path_length(&self, x: &[f64]) -> f64 {
        let mut node = 0usize;
        let mut depth = 0.0f64;
        loop {
            match &self.nodes[node] {
                Node::Leaf { size } => return depth + c_factor(*size),
                Node::Axis {
                    feature,
                    value,
                    left,
                    right,
                } => {
                    depth += 1.0;
                    node = if x[*feature] < *value { *left } else { *right };
                }
                Node::Hyper {
                    normal,
                    offset,
                    left,
                    right,
                } => {
                    depth += 1.0;
                    let mut dot = -*offset;
                    for (k, &f) in self.feats.iter().enumerate() {
                        dot += normal[k] * x[f];
                    }
                    node = if dot <= 0.0 { *left } else { *right };
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn grow(
    nodes: &mut Vec<Node>,
    x: &[Vec<f64>],
    idx: &[usize],
    feats: &[usize],
    depth: usize,
    limit: usize,
    extended: bool,
    rng: &mut Rng,
) -> usize {
    if idx.len() <= 1 || depth >= limit {
        nodes.push(Node::Leaf { size: idx.len() });
        return nodes.len() - 1;
    }

    // Per-feature min/max inside this node.
    let mut lo = vec![f64::INFINITY; feats.len()];
    let mut hi = vec![f64::NEG_INFINITY; feats.len()];
    for &i in idx {
        for (k, &f) in feats.iter().enumerate() {
            let v = x[i][f];
            if v < lo[k] {
                lo[k] = v;
            }
            if v > hi[k] {
                hi[k] = v;
            }
        }
    }
    let spread: Vec<usize> = (0..feats.len()).filter(|&k| hi[k] > lo[k]).collect();
    if spread.is_empty() {
        // Every candidate feature is constant here — the points are duplicates
        // as far as this tree can tell, so they share a leaf.
        nodes.push(Node::Leaf { size: idx.len() });
        return nodes.len() - 1;
    }

    let mut left = Vec::new();
    let mut right = Vec::new();
    let node_kind;

    if extended {
        // Random hyperplane: a Gaussian normal vector plus an intercept drawn
        // inside this node's bounding box. Retry a few times if the cut misses
        // every point (possible with near-degenerate boxes).
        let mut chosen = None;
        for _ in 0..8 {
            let normal: Vec<f64> = (0..feats.len())
                .map(|k| if hi[k] > lo[k] { rng.normal() } else { 0.0 })
                .collect();
            if normal.iter().all(|v| v.abs() < 1e-12) {
                continue;
            }
            let mut offset = 0.0;
            for (k, n) in normal.iter().enumerate() {
                let p = lo[k] + rng.unit() * (hi[k] - lo[k]);
                offset += n * p;
            }
            left.clear();
            right.clear();
            for &i in idx {
                let mut dot = -offset;
                for (k, &f) in feats.iter().enumerate() {
                    dot += normal[k] * x[i][f];
                }
                if dot <= 0.0 {
                    left.push(i);
                } else {
                    right.push(i);
                }
            }
            if !left.is_empty() && !right.is_empty() {
                chosen = Some((normal, offset));
                break;
            }
        }
        let Some((normal, offset)) = chosen else {
            nodes.push(Node::Leaf { size: idx.len() });
            return nodes.len() - 1;
        };
        node_kind = Node::Hyper {
            normal,
            offset,
            left: 0,
            right: 0,
        };
    } else {
        let k = spread[rng.below(spread.len())];
        let feature = feats[k];
        // Uniform in [lo, hi): strictly below hi, so both sides stay non-empty.
        let value = lo[k] + rng.unit() * (hi[k] - lo[k]);
        for &i in idx {
            if x[i][feature] < value {
                left.push(i);
            } else {
                right.push(i);
            }
        }
        if left.is_empty() || right.is_empty() {
            nodes.push(Node::Leaf { size: idx.len() });
            return nodes.len() - 1;
        }
        node_kind = Node::Axis {
            feature,
            value,
            left: 0,
            right: 0,
        };
    }

    let me = nodes.len();
    nodes.push(node_kind);
    let l = grow(nodes, x, &left, feats, depth + 1, limit, extended, rng);
    let r = grow(nodes, x, &right, feats, depth + 1, limit, extended, rng);
    match &mut nodes[me] {
        Node::Axis { left, right, .. } | Node::Hyper { left, right, .. } => {
            *left = l;
            *right = r;
        }
        Node::Leaf { .. } => unreachable!("interior node replaced by a leaf"),
    }
    me
}

// --------------------------------------------------------------- settings ---

/// Resolve `sample_size`: `auto`, an integer count, or a fraction (written with
/// a decimal point or a `%` sign).
fn resolve_sample_size(spec: &str, n: usize) -> Result<(usize, String), String> {
    let raw = spec.trim();
    let low = raw.to_ascii_lowercase();
    if low.is_empty() || low == "auto" {
        return Ok((n.min(256).max(2.min(n)), "auto".into()));
    }
    let (value, fractional) = if let Some(p) = low.strip_suffix('%') {
        let v: f64 = p
            .trim()
            .parse()
            .map_err(|_| format!("sample_size percentage '{raw}' is not a number"))?;
        (v / 100.0, true)
    } else {
        let v: f64 = low.parse().map_err(|_| {
            format!("sample_size must be 'auto', a row count, or a fraction (got '{raw}')")
        })?;
        (v, low.contains('.'))
    };
    if !value.is_finite() || value <= 0.0 {
        return Err(format!("sample_size must be positive (got '{raw}')"));
    }
    let psi = if fractional {
        if value > 1.0 {
            return Err(format!(
                "sample_size fraction must be between 0 and 1 (got '{raw}')"
            ));
        }
        (value * n as f64).ceil() as usize
    } else {
        value.round() as usize
    };
    let psi = psi.clamp(2.min(n), n);
    Ok((psi, format!("from {raw}")))
}

/// Resolve `contamination` into an expected outlier rate, or `None` for `auto`.
fn resolve_contamination(spec: &str) -> Result<Option<f64>, String> {
    let raw = spec.trim();
    let low = raw.to_ascii_lowercase();
    if low.is_empty() || low == "auto" {
        return Ok(None);
    }
    let rate = if let Some(p) = low.strip_suffix('%') {
        p.trim()
            .parse::<f64>()
            .map_err(|_| format!("contamination percentage '{raw}' is not a number"))?
            / 100.0
    } else {
        low.parse::<f64>().map_err(|_| {
            format!("contamination must be 'auto', a fraction like 0.05, or a percentage like 5% (got '{raw}')")
        })?
    };
    if !rate.is_finite() || rate <= 0.0 || rate > 0.5 {
        return Err(format!(
            "contamination must be greater than 0 and at most 0.5 (50% of the rows); got '{raw}'"
        ));
    }
    Ok(Some(rate))
}

// -------------------------------------------------------------- formatting ---

fn fmt(v: f64, d: u32) -> String {
    let s = format!("{:.*}", d as usize, v);
    if s.starts_with("-") && s[1..].chars().all(|c| c == '0' || c == '.') {
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

fn csv_cell(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn pad(s: &str, w: usize) -> String {
    let n = s.chars().count();
    if n >= w {
        s.to_string()
    } else {
        format!("{s}{}", " ".repeat(w - n))
    }
}

fn rpad(s: &str, w: usize) -> String {
    let n = s.chars().count();
    if n >= w {
        s.to_string()
    } else {
        format!("{}{s}", " ".repeat(w - n))
    }
}

// ------------------------------------------------------------------- run ---

struct Scored {
    /// 1-based data-row number (the header row is not counted).
    row_no: usize,
    score: Option<f64>,
    path: Option<f64>,
    flagged: bool,
    rank: Option<usize>,
}

/// Fit an isolation forest on `data` and report per-row anomaly scores.
pub fn run(data: &str, opts: &Options) -> Result<String, String> {
    let method = match opts.method.trim().to_ascii_lowercase().as_str() {
        "" | "standard" => "standard",
        "extended" => "extended",
        other => {
            return Err(format!(
                "method must be 'standard' or 'extended' (got '{other}')"
            ))
        }
    };
    let format = match opts.format.trim().to_ascii_lowercase().as_str() {
        "" | "text" => "text",
        "json" => "json",
        "csv" => "csv",
        other => {
            return Err(format!(
                "format must be 'text', 'json' or 'csv' (got '{other}')"
            ))
        }
    };
    let sort = match opts.sort.trim().to_ascii_lowercase().as_str() {
        "" | "input" => "input",
        "score" => "score",
        other => return Err(format!("sort must be 'input' or 'score' (got '{other}')")),
    };
    let missing = match opts.missing.trim().to_ascii_lowercase().as_str() {
        "" | "drop" => "drop",
        "median" => "median",
        "mean" => "mean",
        "zero" => "zero",
        "error" => "error",
        other => {
            return Err(format!(
                "missing must be 'drop', 'median', 'mean', 'zero' or 'error' (got '{other}')"
            ))
        }
    };
    if opts.trees == 0 || opts.trees > MAX_TREES {
        return Err(format!(
            "trees must be between 1 and {MAX_TREES} (got {})",
            opts.trees
        ));
    }
    if opts.decimals > 12 {
        return Err(format!(
            "decimals must be between 0 and 12 (got {})",
            opts.decimals
        ));
    }
    if !opts.threshold.is_finite() || opts.threshold < 0.0 || opts.threshold >= 1.0 {
        return Err(format!(
            "threshold must be 0 (derive it from contamination) or a score cut-off below 1; got {}",
            opts.threshold
        ));
    }
    let rate = resolve_contamination(&opts.contamination)?;

    let table = parse_table(data, opts)?;
    let feats = resolve_features(&opts.features, &table)?;
    let d = feats.len();
    if !opts.max_features.is_finite() || opts.max_features <= 0.0 {
        return Err(format!(
            "max_features must be a fraction in (0, 1] or a positive column count; got {}",
            opts.max_features
        ));
    }
    let n_feats = if opts.max_features <= 1.0 {
        ((opts.max_features * d as f64).ceil() as usize).clamp(1, d)
    } else {
        (opts.max_features.round() as usize).clamp(1, d)
    };

    // -------- build the feature matrix, applying the missing-cell policy ----
    let mut column_fill = vec![0.0f64; d];
    if missing == "median" || missing == "mean" {
        for (k, &c) in feats.iter().enumerate() {
            let mut vals: Vec<f64> = table.rows.iter().filter_map(|r| parse_num(&r[c])).collect();
            if vals.is_empty() {
                return Err(format!(
                    "column '{}' has no numeric values to impute from",
                    table.names[c]
                ));
            }
            column_fill[k] = if missing == "mean" {
                vals.iter().sum::<f64>() / vals.len() as f64
            } else {
                vals.sort_by(|a, b| a.partial_cmp(b).unwrap());
                let m = vals.len() / 2;
                if vals.len() % 2 == 1 {
                    vals[m]
                } else {
                    (vals[m - 1] + vals[m]) / 2.0
                }
            };
        }
    }

    let mut x: Vec<Vec<f64>> = Vec::new();
    let mut scored_rows: Vec<usize> = Vec::new(); // index into table.rows
    let mut dropped: Vec<usize> = Vec::new(); // 1-based row numbers
    let mut imputed_cells = 0usize;
    for (ri, row) in table.rows.iter().enumerate() {
        let mut vals = Vec::with_capacity(d);
        let mut bad: Option<usize> = None;
        for (k, &c) in feats.iter().enumerate() {
            match parse_num(&row[c]) {
                Some(v) => vals.push(v),
                None => match missing {
                    "drop" => {
                        bad = Some(c);
                        break;
                    }
                    "error" => {
                        return Err(format!(
                            "row {} column '{}' is not a finite number ('{}'). Set missing=drop to skip such rows, or median/mean/zero to fill them.",
                            ri + 1,
                            table.names[c],
                            row[c]
                        ))
                    }
                    "zero" => {
                        imputed_cells += 1;
                        vals.push(0.0);
                    }
                    _ => {
                        imputed_cells += 1;
                        vals.push(column_fill[k]);
                    }
                },
            }
        }
        if bad.is_some() {
            dropped.push(ri + 1);
            continue;
        }
        scored_rows.push(ri);
        x.push(vals);
    }

    let n = x.len();
    if n < 2 {
        return Err(format!(
            "need at least 2 usable rows to fit an isolation forest, found {n} (of {} data rows). Check the feature columns and the missing-value policy.",
            table.rows.len()
        ));
    }

    // -------- extended mode standardizes, because hyperplanes mix columns ---
    let mut standardized = false;
    if method == "extended" {
        standardized = true;
        for k in 0..d {
            let mean = x.iter().map(|r| r[k]).sum::<f64>() / n as f64;
            let var = x.iter().map(|r| (r[k] - mean).powi(2)).sum::<f64>() / n as f64;
            let sd = var.sqrt();
            let sd = if sd > 0.0 { sd } else { 1.0 };
            for r in x.iter_mut() {
                r[k] = (r[k] - mean) / sd;
            }
        }
    }

    let (psi, psi_note) = resolve_sample_size(&opts.sample_size, n)?;
    let limit = (psi as f64).log2().ceil().max(1.0) as usize;

    // -------- fit ----------------------------------------------------------
    let mut rng = Rng::new(opts.seed);
    let mut order: Vec<usize> = (0..n).collect();
    let mut forest: Vec<Tree> = Vec::with_capacity(opts.trees as usize);
    for _ in 0..opts.trees {
        let sample: Vec<usize> = if opts.bootstrap {
            (0..psi).map(|_| rng.below(n)).collect()
        } else {
            // Partial Fisher-Yates: the first psi slots become the subsample.
            for i in 0..psi.min(n.saturating_sub(1)) {
                let j = i + rng.below(n - i);
                order.swap(i, j);
            }
            order[..psi].to_vec()
        };
        let mut tree_feats: Vec<usize> = (0..d).collect();
        if n_feats < d {
            for i in 0..n_feats {
                let j = i + rng.below(d - i);
                tree_feats.swap(i, j);
            }
            tree_feats.truncate(n_feats);
            tree_feats.sort_unstable();
        }
        let mut nodes = Vec::new();
        grow(
            &mut nodes,
            &x,
            &sample,
            &tree_feats,
            0,
            limit,
            method == "extended",
            &mut rng,
        );
        forest.push(Tree {
            nodes,
            feats: tree_feats,
        });
    }

    // -------- score --------------------------------------------------------
    let cpsi = c_factor(psi);
    let mut scores = Vec::with_capacity(n);
    let mut paths = Vec::with_capacity(n);
    for row in &x {
        let total: f64 = forest.iter().map(|t| t.path_length(row)).sum();
        let avg = total / forest.len() as f64;
        paths.push(avg);
        scores.push(if cpsi > 0.0 {
            2f64.powf(-avg / cpsi)
        } else {
            0.5
        });
    }

    // -------- threshold ----------------------------------------------------
    let mut by_score: Vec<usize> = (0..n).collect();
    by_score.sort_by(|&a, &b| {
        scores[b]
            .partial_cmp(&scores[a])
            .unwrap_or(core::cmp::Ordering::Equal)
            .then(a.cmp(&b))
    });
    let (cut, cut_source) = if opts.threshold > 0.0 {
        (
            opts.threshold,
            format!("explicit {}", fmt(opts.threshold, opts.decimals)),
        )
    } else if let Some(p) = rate {
        let k = ((p * n as f64).round() as usize).clamp(1, n);
        (scores[by_score[k - 1]], format!("contamination {}", pct(p)))
    } else {
        (0.5, "auto (0.5)".to_string())
    };

    let flagged: Vec<bool> = scores.iter().map(|&s| s >= cut).collect();
    let n_flagged = flagged.iter().filter(|f| **f).count();

    let mut rank_of = vec![0usize; n];
    for (r, &i) in by_score.iter().enumerate() {
        rank_of[i] = r + 1;
    }

    // -------- assemble the row list ----------------------------------------
    let mut rows: Vec<Scored> = Vec::with_capacity(table.rows.len());
    let mut seen = 0usize;
    for (ri, _) in table.rows.iter().enumerate() {
        if seen < scored_rows.len() && scored_rows[seen] == ri {
            rows.push(Scored {
                row_no: ri + 1,
                score: Some(scores[seen]),
                path: Some(paths[seen]),
                flagged: flagged[seen],
                rank: Some(rank_of[seen]),
            });
            seen += 1;
        } else {
            rows.push(Scored {
                row_no: ri + 1,
                score: None,
                path: None,
                flagged: false,
                rank: None,
            });
        }
    }
    if sort == "score" {
        rows.sort_by(|a, b| match (a.rank, b.rank) {
            (Some(x), Some(y)) => x.cmp(&y),
            (Some(_), None) => core::cmp::Ordering::Less,
            (None, Some(_)) => core::cmp::Ordering::Greater,
            (None, None) => a.row_no.cmp(&b.row_no),
        });
    }
    if opts.only_anomalies {
        rows.retain(|r| r.flagged);
    }
    let listed_total = rows.len();
    if opts.top > 0 && rows.len() > opts.top as usize {
        rows.truncate(opts.top as usize);
    }

    // -------- summary numbers ----------------------------------------------
    let mean_score = scores.iter().sum::<f64>() / n as f64;
    let mean_path = paths.iter().sum::<f64>() / n as f64;
    let mean_of = |want: bool, src: &Vec<f64>| -> Option<f64> {
        let v: Vec<f64> = (0..n)
            .filter(|&i| flagged[i] == want)
            .map(|i| src[i])
            .collect();
        if v.is_empty() {
            None
        } else {
            Some(v.iter().sum::<f64>() / v.len() as f64)
        }
    };
    let s_norm = mean_of(false, &scores);
    let s_anom = mean_of(true, &scores);
    let p_norm = mean_of(false, &paths);
    let p_anom = mean_of(true, &paths);

    let dec = opts.decimals;
    let feat_names: Vec<String> = feats.iter().map(|&c| table.names[c].clone()).collect();

    match format {
        "csv" => {
            let mut out = String::new();
            let mut head: Vec<String> = table.names.iter().map(|n| csv_cell(n)).collect();
            head.push("anomaly_score".into());
            head.push("path_length".into());
            head.push("is_anomaly".into());
            head.push("rank".into());
            out.push_str(&head.join(","));
            out.push('\n');
            for r in &rows {
                let src = &table.rows[r.row_no - 1];
                let mut line: Vec<String> = src.iter().map(|c| csv_cell(c)).collect();
                match (r.score, r.path, r.rank) {
                    (Some(s), Some(p), Some(k)) => {
                        line.push(fmt(s, dec));
                        line.push(fmt(p, dec));
                        line.push(if r.flagged { "yes".into() } else { "no".into() });
                        line.push(k.to_string());
                    }
                    _ => {
                        line.push(String::new());
                        line.push(String::new());
                        line.push(String::new());
                        line.push(String::new());
                    }
                }
                out.push_str(&line.join(","));
                out.push('\n');
            }
            Ok(out)
        }
        "json" => {
            let mut out = String::from("{\n");
            out.push_str(&format!("  \"method\": {},\n", json_str(method)));
            out.push_str(&format!("  \"trees\": {},\n", opts.trees));
            out.push_str(&format!("  \"sample_size\": {psi},\n"));
            out.push_str(&format!("  \"height_limit\": {limit},\n"));
            out.push_str(&format!("  \"bootstrap\": {},\n", opts.bootstrap));
            out.push_str(&format!("  \"max_features\": {n_feats},\n"));
            out.push_str(&format!("  \"seed\": {},\n", opts.seed));
            out.push_str(&format!(
                "  \"features\": [{}],\n",
                feat_names
                    .iter()
                    .map(|n| json_str(n))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
            out.push_str(&format!("  \"rows_scored\": {n},\n"));
            out.push_str(&format!("  \"rows_dropped\": {},\n", dropped.len()));
            out.push_str(&format!("  \"cells_imputed\": {imputed_cells},\n"));
            out.push_str(&format!("  \"standardized\": {standardized},\n"));
            out.push_str(&format!("  \"threshold\": {},\n", fmt(cut, dec)));
            out.push_str(&format!(
                "  \"threshold_source\": {},\n",
                json_str(&cut_source)
            ));
            out.push_str(&format!("  \"anomalies\": {n_flagged},\n"));
            out.push_str(&format!(
                "  \"anomaly_rate\": {},\n",
                fmt(n_flagged as f64 / n as f64, dec)
            ));
            out.push_str(&format!("  \"mean_score\": {},\n", fmt(mean_score, dec)));
            out.push_str(&format!(
                "  \"mean_path_length\": {},\n",
                fmt(mean_path, dec)
            ));
            let opt = |v: Option<f64>| match v {
                Some(x) => fmt(x, dec),
                None => "null".into(),
            };
            out.push_str(&format!("  \"mean_score_normal\": {},\n", opt(s_norm)));
            out.push_str(&format!("  \"mean_score_anomalies\": {},\n", opt(s_anom)));
            out.push_str(&format!("  \"mean_path_normal\": {},\n", opt(p_norm)));
            out.push_str(&format!("  \"mean_path_anomalies\": {},\n", opt(p_anom)));
            out.push_str(&format!("  \"rows_listed\": {},\n", rows.len()));
            out.push_str("  \"rows\": [\n");
            let mut parts = Vec::with_capacity(rows.len());
            for r in &rows {
                let src = &table.rows[r.row_no - 1];
                let values = feats
                    .iter()
                    .map(|&c| json_str(&src[c]))
                    .collect::<Vec<_>>()
                    .join(", ");
                let body = match (r.score, r.path, r.rank) {
                    (Some(s), Some(p), Some(k)) => format!(
                        "\"anomaly_score\": {}, \"path_length\": {}, \"is_anomaly\": {}, \"rank\": {}",
                        fmt(s, dec),
                        fmt(p, dec),
                        r.flagged,
                        k
                    ),
                    _ => "\"anomaly_score\": null, \"path_length\": null, \"is_anomaly\": null, \"rank\": null".into(),
                };
                parts.push(format!(
                    "    {{\"row\": {}, \"values\": [{}], {}}}",
                    r.row_no, values, body
                ));
            }
            out.push_str(&parts.join(",\n"));
            out.push_str("\n  ]\n}\n");
            Ok(out)
        }
        _ => {
            let title = if method == "extended" {
                "Isolation Forest — extended (hyperplane splits)"
            } else {
                "Isolation Forest — standard (axis-parallel splits)"
            };
            let mut out = format!("{title}\n\n");
            out.push_str(&format!(
                "Rows scored:      {n} of {} data rows\n",
                table.rows.len()
            ));
            if !dropped.is_empty() {
                let shown: Vec<String> = dropped.iter().take(10).map(|r| r.to_string()).collect();
                out.push_str(&format!(
                    "Rows dropped:     {} (non-numeric or blank cells) — rows {}{}\n",
                    dropped.len(),
                    shown.join(", "),
                    if dropped.len() > 10 { ", …" } else { "" }
                ));
            }
            if imputed_cells > 0 {
                out.push_str(&format!(
                    "Cells filled:     {imputed_cells} (missing = {missing})\n"
                ));
            }
            out.push_str(&format!(
                "Features:         {} of {} columns — {}\n",
                d,
                table.names.len(),
                feat_names.join(", ")
            ));
            out.push_str(&format!("Trees:            {}\n", opts.trees));
            out.push_str(&format!("Sample size:      {psi} ({psi_note})\n"));
            out.push_str(&format!("Height limit:     {limit}\n"));
            out.push_str(&format!("Features/tree:    {n_feats} of {d}\n"));
            out.push_str(&format!(
                "Bootstrap:        {}\n",
                if opts.bootstrap { "yes" } else { "no" }
            ));
            if standardized {
                out.push_str("Standardized:     yes (extended splits mix columns)\n");
            }
            out.push_str(&format!("Seed:             {}\n", opts.seed));
            out.push_str(&format!(
                "Score threshold:  {} — {cut_source}\n",
                fmt(cut, dec)
            ));
            out.push_str(&format!(
                "Anomalies:        {n_flagged} of {n} rows ({})\n",
                pct(n_flagged as f64 / n as f64)
            ));
            let pair = |all: f64, norm: Option<f64>, anom: Option<f64>| {
                format!(
                    "{} (normal {}, anomalies {})",
                    fmt(all, dec),
                    norm.map(|v| fmt(v, dec)).unwrap_or_else(|| "n/a".into()),
                    anom.map(|v| fmt(v, dec)).unwrap_or_else(|| "n/a".into()),
                )
            };
            out.push_str(&format!(
                "Mean score:       {}\n",
                pair(mean_score, s_norm, s_anom)
            ));
            out.push_str(&format!(
                "Mean path length: {}\n",
                pair(mean_path, p_norm, p_anom)
            ));

            out.push('\n');
            if rows.is_empty() {
                out.push_str("No rows to list.\n");
                return Ok(out);
            }

            // Column widths.
            let score_w = fmt(0.0, dec).len().max(5);
            let rank_w = rows
                .iter()
                .filter_map(|r| r.rank)
                .map(|k| k.to_string().len())
                .max()
                .unwrap_or(1)
                .max(4);
            let row_w = rows
                .iter()
                .map(|r| r.row_no.to_string().len())
                .max()
                .unwrap_or(1)
                .max(3);
            let mut feat_w: Vec<usize> = feat_names.iter().map(|n| n.chars().count()).collect();
            for r in &rows {
                let src = &table.rows[r.row_no - 1];
                for (k, &c) in feats.iter().enumerate() {
                    feat_w[k] = feat_w[k].max(src[c].chars().count());
                }
            }

            let mut head = format!(
                "{} {} {} {} {}",
                rpad("Rank", rank_w),
                rpad("Row", row_w),
                rpad("Score", score_w),
                rpad("Path", score_w.max(6)),
                pad("Flag", 7)
            );
            for (k, name) in feat_names.iter().enumerate() {
                head.push(' ');
                head.push_str(&rpad(name, feat_w[k]));
            }
            out.push_str(head.trim_end());
            out.push('\n');

            for r in &rows {
                let src = &table.rows[r.row_no - 1];
                let mut line = format!(
                    "{} {} {} {} {}",
                    rpad(
                        &r.rank.map(|k| k.to_string()).unwrap_or_else(|| "-".into()),
                        rank_w
                    ),
                    rpad(&r.row_no.to_string(), row_w),
                    rpad(
                        &r.score.map(|s| fmt(s, dec)).unwrap_or_else(|| "-".into()),
                        score_w
                    ),
                    rpad(
                        &r.path.map(|p| fmt(p, dec)).unwrap_or_else(|| "-".into()),
                        score_w.max(6)
                    ),
                    pad(
                        if r.score.is_none() {
                            "skipped"
                        } else if r.flagged {
                            "ANOMALY"
                        } else {
                            "normal"
                        },
                        7
                    )
                );
                for (k, &c) in feats.iter().enumerate() {
                    line.push(' ');
                    line.push_str(&rpad(&src[c], feat_w[k]));
                }
                out.push_str(line.trim_end());
                out.push('\n');
            }
            if listed_total > rows.len() {
                out.push_str(&format!(
                    "\n… {} more rows (top = {}). Use format=csv for the full labelled table.\n",
                    listed_total - rows.len(),
                    opts.top
                ));
            }
            Ok(out)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Nine tightly-clustered rows plus one obvious outlier.
    const SAMPLE: &str = "temp,pressure\n20,101\n21,102\n20,100\n22,101\n21,101\n20,102\n21,100\n22,102\n20,101\n90,300";

    fn opts() -> Options {
        Options::default()
    }

    #[test]
    fn flags_the_obvious_outlier_first() {
        let out = run(SAMPLE, &opts()).unwrap();
        assert!(
            out.contains("Isolation Forest — standard (axis-parallel splits)"),
            "{out}"
        );
        assert!(
            out.contains("Rows scored:      10 of 10 data rows"),
            "{out}"
        );
        assert!(
            out.contains("Features:         2 of 2 columns — temp, pressure"),
            "{out}"
        );
        assert!(
            out.contains("Anomalies:        1 of 10 rows (10.00%)"),
            "{out}"
        );
        // Row 10 is the outlier: rank 1 and the only flagged row.
        let line = out
            .lines()
            .find(|l| l.trim_start().starts_with("1 ") && l.contains("ANOMALY"))
            .unwrap_or_else(|| panic!("no flagged row in\n{out}"));
        assert!(line.contains("90"), "{line}");
        assert_eq!(out.matches("ANOMALY").count(), 1, "{out}");
    }

    #[test]
    fn outlier_scores_above_the_cluster() {
        let out = run(
            SAMPLE,
            &Options {
                format: "json".into(),
                ..opts()
            },
        )
        .unwrap();
        assert!(out.contains("\"rows_scored\": 10"), "{out}");
        assert!(out.contains("\"anomalies\": 1"), "{out}");
        assert!(
            out.contains("\"threshold_source\": \"auto (0.5)\""),
            "{out}"
        );
        let scores: Vec<f64> = out
            .lines()
            .filter_map(|l| l.split("\"anomaly_score\": ").nth(1))
            .filter_map(|t| t.split(',').next())
            .filter_map(|t| t.trim().parse::<f64>().ok())
            .collect();
        assert_eq!(scores.len(), 10, "{out}");
        let last = scores[9];
        assert!(last > 0.5, "outlier score {last} should exceed 0.5\n{out}");
        assert!(
            scores[..9].iter().all(|&s| s < last),
            "every clustered row should score below the outlier: {scores:?}"
        );
    }

    #[test]
    fn same_seed_is_reproducible_and_different_seeds_agree_on_the_outlier() {
        let a = run(SAMPLE, &opts()).unwrap();
        let b = run(SAMPLE, &opts()).unwrap();
        assert_eq!(a, b, "same seed must be byte-identical");
        let c = run(
            SAMPLE,
            &Options {
                seed: 12345,
                ..opts()
            },
        )
        .unwrap();
        assert_ne!(a, c, "a different seed should change the scores");
        assert_eq!(c.matches("ANOMALY").count(), 1, "{c}");
    }

    #[test]
    fn contamination_sets_the_flagged_count() {
        let out = run(
            SAMPLE,
            &Options {
                contamination: "20%".into(),
                ..opts()
            },
        )
        .unwrap();
        assert!(out.contains("contamination 20.00%"), "{out}");
        assert!(
            out.contains("Anomalies:        2 of 10 rows (20.00%)"),
            "{out}"
        );
    }

    #[test]
    fn explicit_threshold_overrides_contamination() {
        let out = run(
            SAMPLE,
            &Options {
                contamination: "50%".into(),
                threshold: 0.99,
                ..opts()
            },
        )
        .unwrap();
        assert!(out.contains("explicit 0.9900"), "{out}");
        assert!(
            out.contains("Anomalies:        0 of 10 rows (0.00%)"),
            "{out}"
        );
    }

    #[test]
    fn csv_output_keeps_every_input_column() {
        let out = run(
            SAMPLE,
            &Options {
                format: "csv".into(),
                ..opts()
            },
        )
        .unwrap();
        let mut lines = out.lines();
        assert_eq!(
            lines.next().unwrap(),
            "temp,pressure,anomaly_score,path_length,is_anomaly,rank"
        );
        assert_eq!(out.lines().count(), 11);
        let last = out.lines().last().unwrap();
        assert!(last.starts_with("90,300,"), "{last}");
        assert!(last.contains(",yes,1"), "{last}");
    }

    #[test]
    fn sort_and_top_rank_the_most_anomalous_rows() {
        let out = run(
            SAMPLE,
            &Options {
                sort: "score".into(),
                top: 3,
                format: "csv".into(),
                ..opts()
            },
        )
        .unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), 4, "header + 3 rows: {out}");
        assert!(lines[1].starts_with("90,300,"), "{out}");
        assert!(lines[1].ends_with(",yes,1"), "{out}");
    }

    #[test]
    fn only_anomalies_lists_just_the_flagged_rows() {
        let out = run(
            SAMPLE,
            &Options {
                only_anomalies: true,
                ..opts()
            },
        )
        .unwrap();
        let data_lines: Vec<&str> = out.lines().filter(|l| l.contains(" ANOMALY")).collect();
        assert_eq!(data_lines.len(), 1, "{out}");
        assert!(data_lines[0].contains("90"), "{out}");
    }

    #[test]
    fn extended_method_standardizes_and_still_finds_the_outlier() {
        let out = run(
            SAMPLE,
            &Options {
                method: "extended".into(),
                ..opts()
            },
        )
        .unwrap();
        assert!(out.contains("extended (hyperplane splits)"), "{out}");
        assert!(out.contains("Standardized:     yes"), "{out}");
        assert_eq!(out.matches("ANOMALY").count(), 1, "{out}");
    }

    #[test]
    fn missing_cells_are_dropped_by_default_and_can_be_imputed() {
        let data = "temp,pressure\n20,101\n21,\n20,100\n22,101\n21,101\n90,300";
        let dropped = run(data, &opts()).unwrap();
        assert!(
            dropped.contains("Rows scored:      5 of 6 data rows"),
            "{dropped}"
        );
        assert!(dropped.contains("Rows dropped:     1"), "{dropped}");
        assert!(dropped.contains("skipped"), "{dropped}");

        let filled = run(
            data,
            &Options {
                missing: "median".into(),
                ..opts()
            },
        )
        .unwrap();
        assert!(
            filled.contains("Rows scored:      6 of 6 data rows"),
            "{filled}"
        );
        assert!(
            filled.contains("Cells filled:     1 (missing = median)"),
            "{filled}"
        );
    }

    #[test]
    fn features_can_be_named_or_indexed_and_extra_columns_ignored() {
        let data = "id,temp,label\n1,20,ok\n2,21,ok\n3,20,ok\n4,22,ok\n5,90,ok";
        let by_name = run(
            data,
            &Options {
                features: "temp".into(),
                ..opts()
            },
        )
        .unwrap();
        assert!(
            by_name.contains("Features:         1 of 3 columns — temp"),
            "{by_name}"
        );
        let by_index = run(
            data,
            &Options {
                features: "2".into(),
                ..opts()
            },
        )
        .unwrap();
        assert_eq!(by_name, by_index);
        // Auto-selection takes every numeric column, so `id` joins in.
        let auto = run(data, &opts()).unwrap();
        assert!(
            auto.contains("Features:         2 of 3 columns — id, temp"),
            "{auto}"
        );
    }

    #[test]
    fn whitespace_and_semicolon_tables_parse() {
        let semi = "a;b\n1;1\n2;1\n1;2\n2;2\n50;50";
        let out = run(semi, &opts()).unwrap();
        assert!(out.contains("Rows scored:      5 of 5 data rows"), "{out}");
        let space = "1 1\n2 1\n1 2\n2 2\n50 50";
        let out = run(space, &opts()).unwrap();
        assert!(
            out.contains("Features:         2 of 2 columns — c1, c2"),
            "{out}"
        );
    }

    #[test]
    fn err_on_empty_input() {
        let e = run("   ", &opts()).unwrap_err();
        assert!(e.contains("no data"), "{e}");
    }

    #[test]
    fn err_on_ragged_rows() {
        let e = run("a,b\n1,2\n3", &opts()).unwrap_err();
        assert!(e.contains("row 3 has 1 columns"), "{e}");
    }

    #[test]
    fn err_when_no_numeric_columns() {
        let e = run("a,b\nx,y\np,q\nm,n", &opts()).unwrap_err();
        assert!(e.contains("no numeric columns"), "{e}");
    }

    #[test]
    fn err_on_unknown_feature_column() {
        let e = run(
            SAMPLE,
            &Options {
                features: "humidity".into(),
                ..opts()
            },
        )
        .unwrap_err();
        assert!(e.contains("no column named 'humidity'"), "{e}");
        assert!(e.contains("temp, pressure"), "{e}");
    }

    #[test]
    fn err_on_too_few_usable_rows() {
        let e = run("a,b\n1,2", &opts()).unwrap_err();
        assert!(e.contains("at least 2 usable rows"), "{e}");
    }

    #[test]
    fn err_on_bad_enums_and_ranges() {
        assert!(run(
            SAMPLE,
            &Options {
                method: "deep".into(),
                ..opts()
            }
        )
        .unwrap_err()
        .contains("method must be"));
        assert!(run(
            SAMPLE,
            &Options {
                format: "xml".into(),
                ..opts()
            }
        )
        .unwrap_err()
        .contains("format must be"));
        assert!(run(
            SAMPLE,
            &Options {
                sort: "random".into(),
                ..opts()
            }
        )
        .unwrap_err()
        .contains("sort must be"));
        assert!(run(
            SAMPLE,
            &Options {
                missing: "guess".into(),
                ..opts()
            }
        )
        .unwrap_err()
        .contains("missing must be"));
        assert!(run(SAMPLE, &Options { trees: 0, ..opts() })
            .unwrap_err()
            .contains("trees must be between 1"));
        assert!(run(
            SAMPLE,
            &Options {
                contamination: "80%".into(),
                ..opts()
            }
        )
        .unwrap_err()
        .contains("at most 0.5"));
        assert!(run(
            SAMPLE,
            &Options {
                threshold: 1.5,
                ..opts()
            }
        )
        .unwrap_err()
        .contains("threshold must be"));
        assert!(run(
            SAMPLE,
            &Options {
                sample_size: "half".into(),
                ..opts()
            }
        )
        .unwrap_err()
        .contains("sample_size must be"));
    }

    #[test]
    fn missing_error_mode_names_the_offending_cell() {
        let data = "temp,pressure\n20,101\n21,oops\n20,100";
        let e = run(
            data,
            &Options {
                missing: "error".into(),
                features: "temp,pressure".into(),
                ..opts()
            },
        )
        .unwrap_err();
        assert!(e.contains("row 2 column 'pressure'"), "{e}");
        assert!(e.contains("oops"), "{e}");
    }

    #[test]
    fn bootstrap_and_max_features_are_accepted() {
        let out = run(
            SAMPLE,
            &Options {
                bootstrap: true,
                max_features: 0.5,
                trees: 50,
                sample_size: "8".into(),
                ..opts()
            },
        )
        .unwrap();
        assert!(out.contains("Bootstrap:        yes"), "{out}");
        assert!(out.contains("Features/tree:    1 of 2"), "{out}");
        assert!(out.contains("Sample size:      8 (from 8)"), "{out}");
        assert!(out.contains("Trees:            50"), "{out}");
    }

    #[test]
    fn sample_size_accepts_fractions_and_caps_at_the_row_count() {
        let out = run(
            SAMPLE,
            &Options {
                sample_size: "0.5".into(),
                ..opts()
            },
        )
        .unwrap();
        assert!(out.contains("Sample size:      5 (from 0.5)"), "{out}");
        let out = run(
            SAMPLE,
            &Options {
                sample_size: "500".into(),
                ..opts()
            },
        )
        .unwrap();
        assert!(out.contains("Sample size:      10 (from 500)"), "{out}");
        let out = run(
            SAMPLE,
            &Options {
                sample_size: "30%".into(),
                ..opts()
            },
        )
        .unwrap();
        assert!(out.contains("Sample size:      3 (from 30%)"), "{out}");
    }

    #[test]
    fn header_can_be_forced_off_and_quoted_cells_survive() {
        let data = "\"a, b\",c\n1,1\n2,1\n1,2\n2,2\n50,50";
        let out = run(data, &opts()).unwrap();
        assert!(out.contains("— a, b, c"), "{out}");
        let csv = run(
            data,
            &Options {
                format: "csv".into(),
                ..opts()
            },
        )
        .unwrap();
        assert!(csv.starts_with("\"a, b\",c,anomaly_score"), "{csv}");

        let forced = run(
            "1,1\n2,1\n1,2\n2,2\n50,50",
            &Options {
                header: "no".into(),
                ..opts()
            },
        )
        .unwrap();
        assert!(
            forced.contains("Rows scored:      5 of 5 data rows"),
            "{forced}"
        );
    }

    #[test]
    fn c_factor_matches_the_paper() {
        assert_eq!(c_factor(0), 0.0);
        assert_eq!(c_factor(1), 0.0);
        assert_eq!(c_factor(2), 1.0);
        // c(256) ≈ 10.244770920119917 with the harmonic-number approximation.
        assert!((c_factor(256) - 10.244_770_920_119_917).abs() < 1e-12);
    }

    #[test]
    fn decimals_control_the_printed_precision() {
        let out = run(
            SAMPLE,
            &Options {
                decimals: 2,
                format: "csv".into(),
                ..opts()
            },
        )
        .unwrap();
        let last = out.lines().last().unwrap();
        let score = last.split(',').nth(2).unwrap();
        assert_eq!(score.len(), 4, "expected 0.NN, got {score}");
    }

    #[test]
    fn every_row_gets_a_distinct_rank() {
        let out = run(
            SAMPLE,
            &Options {
                format: "csv".into(),
                ..opts()
            },
        )
        .unwrap();
        let mut ranks: Vec<usize> = out
            .lines()
            .skip(1)
            .filter_map(|l| l.rsplit(',').next())
            .filter_map(|t| t.parse().ok())
            .collect();
        ranks.sort_unstable();
        assert_eq!(ranks, (1..=10).collect::<Vec<usize>>());
    }
}
