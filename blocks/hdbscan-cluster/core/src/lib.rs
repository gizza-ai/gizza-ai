//! hdbscan-cluster core — pure compute, shared by the chat skill block and the
//! web page. No wafer/wasm-bindgen deps, no external crates.
//!
//! Runs **HDBSCAN\*** (Campello, Moulavi & Sander, 2013) over the numeric
//! columns of a pasted CSV/TSV table: a hierarchical, density-based clustering
//! that discovers the number of clusters by itself, tolerates clusters of
//! *different* densities, and labels the leftovers as noise.
//!
//! The pipeline is the standard one:
//!
//! 1. **Core distance** — for each point, the distance to its `min_samples`-th
//!    nearest neighbour (the point itself counts as the first neighbour). This
//!    is the local density estimate.
//! 2. **Mutual reachability** — `max(core(a), core(b), d(a, b) / alpha)`. It
//!    pushes sparse points away from each other without moving dense ones,
//!    which is what makes the result robust to single stray points.
//! 3. **Minimum spanning tree** over that metric (exact Prim, `O(n²)`), then a
//!    **single-linkage hierarchy** by adding the MST edges shortest-first.
//! 4. **Condense** the hierarchy: at every split, a side holding fewer than
//!    `min_cluster_size` points is not a new cluster — those points simply
//!    "fall out" of the parent at that density level (lambda = 1 / distance).
//! 5. **Select** clusters from the condensed tree, either by *excess of mass*
//!    (the default: keep a cluster when it is more stable than its descendants
//!    combined) or by taking every *leaf*.
//!
//! Alongside the flat labels the run reports the three things that make
//! HDBSCAN more informative than a single-scale DBSCAN:
//!
//! - **membership probability** — how firmly a point belongs to its cluster
//!   (`lambda(p) / max lambda in the cluster`; noise is 0),
//! - **GLOSH outlier score** — how outlying a point is relative to the density
//!   level of the cluster it detached from (higher = more outlying),
//! - **cluster persistence** — how stable a cluster is across density levels,
//!   normalised into `0..1`.
//!
//! Noise is labelled `-1`, matching the scikit-learn / `hdbscan` convention.
//!
//! HDBSCAN\* has no randomised step, so there is no seed: the same table plus
//! the same settings always produces byte-identical output.

/// Maximum number of data rows (header excluded) accepted in one run. The exact
/// minimum spanning tree is `O(n²)`, so this cap is what keeps a browser run
/// interactive.
pub const MAX_ROWS: usize = 5_000;
/// Maximum number of columns in the pasted table.
pub const MAX_COLS: usize = 200;

/// Options resolved from the tool params.
pub struct Options {
    /// Comma-separated feature columns: header names or 1-based indices.
    /// Blank = every fully numeric column.
    pub features: String,
    /// Smallest group of points that may be called a cluster.
    pub min_cluster_size: u32,
    /// Neighbourhood size `k` for the core distance. 0 = mirror
    /// `min_cluster_size` (the reference default).
    pub min_samples: u32,
    /// Distance metric: euclidean | manhattan | chebyshev | cosine.
    pub metric: String,
    /// Robust-single-linkage distance scaling. 1.0 = plain mutual reachability.
    pub alpha: f64,
    /// Distance below which clusters are merged back together. 0 = off.
    pub cluster_selection_epsilon: f64,
    /// Cluster selection from the condensed tree: "eom" or "leaf".
    pub selection: String,
    /// Allow the whole dataset to be reported as one cluster.
    pub allow_single_cluster: bool,
    /// Excess-of-mass size cap; a cluster above it is split. 0 = no cap.
    pub max_cluster_size: u32,
    /// Standardize each feature to zero mean / unit variance before clustering.
    pub normalize: bool,
    /// Non-numeric / blank cell policy: drop | median | mean | zero | error.
    pub missing: String,
    /// Row order in the output: "input", "cluster" or "outlier".
    pub sort: String,
    /// Keep only the first N listed rows. 0 = all of them.
    pub top: u32,
    /// List only the rows labelled as noise.
    pub only_noise: bool,
    /// Whether the first row holds column names: auto | yes | no.
    pub header: String,
    /// Column delimiter: auto | comma | tab | semicolon | pipe | space, or a
    /// single character.
    pub delimiter: String,
    /// Decimal places for probabilities, outlier scores and centroids.
    pub decimals: u32,
    /// Output format: text | json | csv.
    pub format: String,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            features: String::new(),
            min_cluster_size: 5,
            min_samples: 0,
            metric: "euclidean".into(),
            alpha: 1.0,
            cluster_selection_epsilon: 0.0,
            selection: "eom".into(),
            allow_single_cluster: false,
            max_cluster_size: 0,
            normalize: true,
            missing: "drop".into(),
            sort: "input".into(),
            top: 0,
            only_noise: false,
            header: "auto".into(),
            delimiter: "auto".into(),
            decimals: 4,
            format: "text".into(),
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
}

fn parse_table(data: &str, opts: &Options) -> Result<Table, String> {
    let lines: Vec<&str> = data
        .lines()
        .map(|l| l.trim_end_matches('\r'))
        .filter(|l| !l.trim().is_empty())
        .collect();
    let Some(first) = lines.first() else {
        return Err(
            "no data: paste a table with one row per point, e.g. 'x,y' then '1,1'".into(),
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
            "too many rows: {} (max {MAX_ROWS}). HDBSCAN builds an exact minimum spanning tree, which is quadratic in the row count — sample the table first, or run it on chunks.",
            rows.len()
        ));
    }
    Ok(Table { names, rows })
}

/// Resolve the `features` selector into column indices.
fn resolve_features(spec: &str, t: &Table) -> Result<Vec<usize>, String> {
    let spec = spec.trim();
    if spec.is_empty() {
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
                "no numeric columns found — HDBSCAN needs at least one numeric feature column. Name the columns explicitly with 'features' if they carry units or thousands separators."
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

// --------------------------------------------------------------- distance ---

#[derive(Clone, Copy, PartialEq)]
enum Metric {
    Euclidean,
    Manhattan,
    Chebyshev,
    Cosine,
}

impl Metric {
    fn parse(s: &str) -> Result<Metric, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "euclidean" | "l2" => Ok(Metric::Euclidean),
            "manhattan" | "cityblock" | "l1" => Ok(Metric::Manhattan),
            "chebyshev" | "chebychev" | "linf" => Ok(Metric::Chebyshev),
            "cosine" => Ok(Metric::Cosine),
            other => Err(format!(
                "metric must be 'euclidean', 'manhattan', 'chebyshev' or 'cosine' (got '{other}')"
            )),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Metric::Euclidean => "euclidean",
            Metric::Manhattan => "manhattan",
            Metric::Chebyshev => "chebyshev",
            Metric::Cosine => "cosine",
        }
    }
}

/// Distance between two rows. Cosine returns `1 - cos(a, b)` in `0..2`; a
/// zero-length vector has no direction, so it is defined as maximally distant
/// from any non-zero vector and identical to another zero vector.
fn dist(a: &[f64], b: &[f64], m: Metric, norms: &[f64], i: usize, j: usize) -> f64 {
    match m {
        Metric::Euclidean => a
            .iter()
            .zip(b)
            .map(|(x, y)| (x - y) * (x - y))
            .sum::<f64>()
            .sqrt(),
        Metric::Manhattan => a.iter().zip(b).map(|(x, y)| (x - y).abs()).sum::<f64>(),
        Metric::Chebyshev => a
            .iter()
            .zip(b)
            .map(|(x, y)| (x - y).abs())
            .fold(0.0f64, f64::max),
        Metric::Cosine => {
            let (na, nb) = (norms[i], norms[j]);
            if na == 0.0 || nb == 0.0 {
                return if na == nb { 0.0 } else { 1.0 };
            }
            let dot: f64 = a.iter().zip(b).map(|(x, y)| x * y).sum();
            (1.0 - dot / (na * nb)).clamp(0.0, 2.0)
        }
    }
}

// ---------------------------------------------- single-linkage union-find ---

/// The union-find the single-linkage step needs: every union creates a NEW node
/// id (`n`, `n+1`, …) so the merge sequence is a binary hierarchy.
struct LinkUf {
    parent: Vec<usize>,
    size: Vec<usize>,
    next_label: usize,
}

const NO_PARENT: usize = usize::MAX;

impl LinkUf {
    fn new(n: usize) -> Self {
        let mut size = vec![0usize; 2 * n - 1];
        size[..n].fill(1);
        LinkUf {
            parent: vec![NO_PARENT; 2 * n - 1],
            size,
            next_label: n,
        }
    }

    fn find(&mut self, x: usize) -> usize {
        let mut root = x;
        while self.parent[root] != NO_PARENT {
            root = self.parent[root];
        }
        let mut cur = x;
        while self.parent[cur] != NO_PARENT {
            let next = self.parent[cur];
            self.parent[cur] = root;
            cur = next;
        }
        root
    }

    fn union(&mut self, a: usize, b: usize) {
        let label = self.next_label;
        self.next_label += 1;
        self.size[label] = self.size[a] + self.size[b];
        self.parent[a] = label;
        self.parent[b] = label;
    }
}

/// Plain union-find over condensed-tree node ids, used to collapse every
/// unselected node into the selected ancestor that owns its points.
struct FlatUf {
    parent: Vec<usize>,
}

impl FlatUf {
    fn new(n: usize) -> Self {
        FlatUf {
            parent: (0..n).collect(),
        }
    }
    fn find(&mut self, x: usize) -> usize {
        let mut root = x;
        while self.parent[root] != root {
            root = self.parent[root];
        }
        let mut cur = x;
        while self.parent[cur] != cur {
            let next = self.parent[cur];
            self.parent[cur] = root;
            cur = next;
        }
        root
    }
    /// Attach `child` under `parent` (direction matters: the parent must stay
    /// the representative so `find` returns the enclosing cluster).
    fn union(&mut self, parent: usize, child: usize) {
        let (p, c) = (self.find(parent), self.find(child));
        if p != c {
            self.parent[c] = p;
        }
    }
}

// ----------------------------------------------------------- condensed tree ---

/// One edge of the condensed cluster tree: `child` left `parent` at density
/// level `lambda`. `child` below `n_points` is a single point; at or above it
/// is a sub-cluster of `size` points.
#[derive(Clone, Copy)]
struct CondRow {
    parent: usize,
    child: usize,
    lambda: f64,
    size: usize,
}

/// Every node id in the subtree rooted at `root`, parents before children.
fn bfs_hierarchy(h: &[(usize, usize, f64, usize)], n: usize, root: usize) -> Vec<usize> {
    let mut frontier = vec![root];
    let mut out = Vec::new();
    while !frontier.is_empty() {
        out.extend_from_slice(&frontier);
        let mut next = Vec::new();
        for &node in &frontier {
            if node >= n {
                let (l, r, _, _) = h[node - n];
                next.push(l);
                next.push(r);
            }
        }
        frontier = next;
    }
    out
}

/// Every condensed-tree cluster node in the subtree rooted at `root`.
fn bfs_condensed(rows: &[CondRow], n: usize, root: usize) -> Vec<usize> {
    let mut frontier = vec![root];
    let mut out = Vec::new();
    while !frontier.is_empty() {
        out.extend_from_slice(&frontier);
        let mut next = Vec::new();
        for &node in &frontier {
            for r in rows {
                if r.parent == node && r.child >= n {
                    next.push(r.child);
                }
            }
        }
        frontier = next;
    }
    out
}

/// Walk the single-linkage hierarchy top-down and drop every branch smaller
/// than `min_cluster_size`, turning it into points falling out of the parent.
fn condense(h: &[(usize, usize, f64, usize)], n: usize, min_cluster_size: usize) -> Vec<CondRow> {
    let root = 2 * n - 2;
    let mut relabel = vec![0usize; 2 * n - 1];
    relabel[root] = n;
    let mut next_label = n + 1;
    let mut ignore = vec![false; 2 * n - 1];
    let mut out: Vec<CondRow> = Vec::new();

    for node in bfs_hierarchy(h, n, root) {
        if ignore[node] || node < n {
            continue;
        }
        let (left, right, d, _) = h[node - n];
        let lambda = if d > 0.0 { 1.0 / d } else { f64::INFINITY };
        let left_count = if left >= n { h[left - n].3 } else { 1 };
        let right_count = if right >= n { h[right - n].3 } else { 1 };

        let node_label = relabel[node];
        let fall_out = |out: &mut Vec<CondRow>, ignore: &mut Vec<bool>, branch: usize| {
            for sub in bfs_hierarchy(h, n, branch) {
                if sub < n {
                    out.push(CondRow {
                        parent: node_label,
                        child: sub,
                        lambda,
                        size: 1,
                    });
                }
                ignore[sub] = true;
            }
        };

        if left_count >= min_cluster_size && right_count >= min_cluster_size {
            relabel[left] = next_label;
            next_label += 1;
            out.push(CondRow {
                parent: relabel[node],
                child: relabel[left],
                lambda,
                size: left_count,
            });
            relabel[right] = next_label;
            next_label += 1;
            out.push(CondRow {
                parent: relabel[node],
                child: relabel[right],
                lambda,
                size: right_count,
            });
        } else if left_count < min_cluster_size && right_count < min_cluster_size {
            fall_out(&mut out, &mut ignore, left);
            fall_out(&mut out, &mut ignore, right);
        } else if left_count < min_cluster_size {
            relabel[right] = relabel[node];
            fall_out(&mut out, &mut ignore, left);
        } else {
            relabel[left] = relabel[node];
            fall_out(&mut out, &mut ignore, right);
        }
    }
    out
}

/// Cluster stability: how much "mass" a cluster accumulates between the density
/// level it is born at and the levels at which its members leave it.
fn compute_stability(rows: &[CondRow], n: usize) -> (Vec<f64>, usize) {
    let max_node = rows.iter().map(|r| r.parent.max(r.child)).max().unwrap_or(n);
    let mut births = vec![f64::NAN; max_node + 1];
    for r in rows {
        if r.child >= n {
            let b = &mut births[r.child];
            if b.is_nan() || r.lambda < *b {
                *b = r.lambda;
            }
        }
    }
    births[n] = 0.0;
    let mut stability = vec![0.0f64; max_node + 1];
    for r in rows {
        let birth = births[r.parent];
        if birth.is_nan() {
            continue;
        }
        stability[r.parent] += (r.lambda - birth) * r.size as f64;
    }
    (stability, max_node)
}

/// The condensed-tree rows that describe sub-CLUSTERS (not individual points).
fn cluster_rows(rows: &[CondRow], n: usize) -> Vec<CondRow> {
    rows.iter().filter(|r| r.child >= n).copied().collect()
}

fn birth_eps(ctree: &[CondRow], node: usize) -> f64 {
    ctree
        .iter()
        .find(|r| r.child == node)
        .map(|r| if r.lambda > 0.0 { 1.0 / r.lambda } else { 0.0 })
        .unwrap_or(f64::INFINITY)
}

/// Climb towards the root while the parent is still tighter than `epsilon`.
fn traverse_upwards(ctree: &[CondRow], epsilon: f64, allow_single: bool, leaf: usize, root: usize) -> usize {
    let mut node = leaf;
    loop {
        let Some(parent) = ctree.iter().find(|r| r.child == node).map(|r| r.parent) else {
            return node;
        };
        if parent == root {
            return if allow_single { root } else { node };
        }
        if birth_eps(ctree, parent) > epsilon {
            return parent;
        }
        node = parent;
    }
}

/// Replace every selected cluster that is tighter than `epsilon` with the
/// highest ancestor still within that distance, merging micro-clusters.
fn epsilon_search(
    leaves: &[usize],
    ctree: &[CondRow],
    n: usize,
    epsilon: f64,
    allow_single: bool,
    root: usize,
) -> Vec<usize> {
    let mut selected: Vec<usize> = Vec::new();
    let mut processed: Vec<usize> = Vec::new();
    for &leaf in leaves {
        if birth_eps(ctree, leaf) < epsilon {
            if !processed.contains(&leaf) {
                let replacement = traverse_upwards(ctree, epsilon, allow_single, leaf, root);
                if !selected.contains(&replacement) {
                    selected.push(replacement);
                }
                for sub in bfs_condensed(ctree, n, replacement) {
                    if sub != replacement && !processed.contains(&sub) {
                        processed.push(sub);
                    }
                }
            }
        } else if !selected.contains(&leaf) {
            selected.push(leaf);
        }
    }
    selected.retain(|c| !processed.contains(c) || *c == root);
    selected.sort_unstable();
    selected.dedup();
    selected
}

/// Condensed-tree nodes with no sub-cluster children.
fn cluster_tree_leaves(ctree: &[CondRow], root: usize) -> Vec<usize> {
    let parents: Vec<usize> = ctree.iter().map(|r| r.parent).collect();
    let mut leaves: Vec<usize> = ctree
        .iter()
        .map(|r| r.child)
        .filter(|c| !parents.contains(c))
        .collect();
    leaves.sort_unstable();
    leaves.dedup();
    if leaves.is_empty() {
        leaves.push(root);
    }
    leaves
}

// -------------------------------------------------------------- formatting ---

fn fmt(v: f64, d: u32) -> String {
    let s = format!("{:.*}", d as usize, v);
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

/// Render a right-aligned column table with a header row.
fn table(headers: &[String], rows: &[Vec<String>], left_align: &[bool]) -> String {
    let mut widths: Vec<usize> = headers.iter().map(|h| h.chars().count()).collect();
    for r in rows {
        for (i, c) in r.iter().enumerate() {
            widths[i] = widths[i].max(c.chars().count());
        }
    }
    let mut out = String::new();
    let render = |cells: &[String], out: &mut String| {
        let mut line = String::new();
        for (i, c) in cells.iter().enumerate() {
            if i > 0 {
                line.push_str("  ");
            }
            line.push_str(&if left_align.get(i).copied().unwrap_or(false) {
                pad(c, widths[i])
            } else {
                rpad(c, widths[i])
            });
        }
        out.push_str(line.trim_end());
        out.push('\n');
    };
    render(headers, &mut out);
    for r in rows {
        render(r, &mut out);
    }
    out
}

// ------------------------------------------------------------------- run ---

/// A clustered cluster, as reported in the summary.
struct ClusterInfo {
    label: usize,
    size: usize,
    persistence: f64,
    centroid: Vec<f64>,
    /// Index into the scored-row list of the point closest to the centroid.
    medoid_row: usize,
}

struct Listed {
    /// 1-based data-row number (the header row is not counted).
    row_no: usize,
    cluster: Option<i64>,
    probability: Option<f64>,
    outlier: Option<f64>,
}

/// Cluster `data` with HDBSCAN* and report per-row labels, probabilities and
/// outlier scores.
pub fn run(data: &str, opts: &Options) -> Result<String, String> {
    // -------- validate the option surface first (cheap, clear errors) -------
    let metric = Metric::parse(&opts.metric)?;
    let selection = match opts.selection.trim().to_ascii_lowercase().as_str() {
        "" | "eom" | "excess_of_mass" => "eom",
        "leaf" => "leaf",
        other => {
            return Err(format!(
                "selection must be 'eom' (excess of mass) or 'leaf' (got '{other}')"
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
        "cluster" => "cluster",
        "outlier" => "outlier",
        other => {
            return Err(format!(
                "sort must be 'input', 'cluster' or 'outlier' (got '{other}')"
            ))
        }
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
    if opts.min_cluster_size < 2 {
        return Err(format!(
            "min_cluster_size must be at least 2 (got {}) — a single point is never a cluster",
            opts.min_cluster_size
        ));
    }
    if opts.min_cluster_size as usize > MAX_ROWS {
        return Err(format!(
            "min_cluster_size must be at most {MAX_ROWS} (got {})",
            opts.min_cluster_size
        ));
    }
    if !opts.alpha.is_finite() || opts.alpha <= 0.0 {
        return Err(format!(
            "alpha must be a positive distance scale, e.g. 1.0 (got {})",
            opts.alpha
        ));
    }
    if !opts.cluster_selection_epsilon.is_finite() || opts.cluster_selection_epsilon < 0.0 {
        return Err(format!(
            "cluster_selection_epsilon must be 0 (off) or a positive distance (got {})",
            opts.cluster_selection_epsilon
        ));
    }
    if opts.decimals > 12 {
        return Err(format!(
            "decimals must be between 0 and 12 (got {})",
            opts.decimals
        ));
    }

    // -------- table -> feature matrix --------------------------------------
    let t = parse_table(data, opts)?;
    let feats = resolve_features(&opts.features, &t)?;
    let d = feats.len();

    let mut column_fill = vec![0.0f64; d];
    if missing == "median" || missing == "mean" {
        for (k, &c) in feats.iter().enumerate() {
            let mut vals: Vec<f64> = t.rows.iter().filter_map(|r| parse_num(&r[c])).collect();
            if vals.is_empty() {
                return Err(format!(
                    "column '{}' has no numeric values to impute from",
                    t.names[c]
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

    let mut raw: Vec<Vec<f64>> = Vec::new();
    let mut scored_rows: Vec<usize> = Vec::new();
    let mut imputed_cells = 0usize;
    for (ri, row) in t.rows.iter().enumerate() {
        let mut vals = Vec::with_capacity(d);
        let mut skip = false;
        for (k, &c) in feats.iter().enumerate() {
            match parse_num(&row[c]) {
                Some(v) => vals.push(v),
                None => match missing {
                    "drop" => {
                        skip = true;
                        break;
                    }
                    "error" => {
                        return Err(format!(
                            "row {} column '{}' is not a finite number ('{}'). Set missing=drop to skip such rows, or median/mean/zero to fill them.",
                            ri + 1,
                            t.names[c],
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
        if skip {
            continue;
        }
        scored_rows.push(ri);
        raw.push(vals);
    }

    let n = raw.len();
    let min_cluster_size = opts.min_cluster_size as usize;
    if n < min_cluster_size.max(2) {
        return Err(format!(
            "need at least {} usable rows to form a cluster of min_cluster_size {}, found {n} (of {} data rows). Lower min_cluster_size, or check the feature columns and the missing-value policy.",
            min_cluster_size.max(2),
            min_cluster_size,
            t.rows.len()
        ));
    }

    // -------- optional standardization -------------------------------------
    let mut x = raw.clone();
    if opts.normalize {
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
    let norms: Vec<f64> = if metric == Metric::Cosine {
        x.iter()
            .map(|r| r.iter().map(|v| v * v).sum::<f64>().sqrt())
            .collect()
    } else {
        Vec::new()
    };

    // -------- core distances ------------------------------------------------
    let k = if opts.min_samples == 0 {
        min_cluster_size
    } else {
        opts.min_samples as usize
    }
    .clamp(1, n);
    let mut core = vec![0.0f64; n];
    // k smallest distances from i (self included, so the first is always 0).
    let mut knn: Vec<f64> = Vec::with_capacity(k);
    for i in 0..n {
        knn.clear();
        for j in 0..n {
            let dij = dist(&x[i], &x[j], metric, &norms, i, j);
            if knn.len() < k {
                let pos = knn.partition_point(|&v| v <= dij);
                knn.insert(pos, dij);
            } else if dij < knn[k - 1] {
                let pos = knn.partition_point(|&v| v <= dij);
                knn.insert(pos, dij);
                knn.pop();
            }
        }
        core[i] = knn[knn.len() - 1];
    }

    let alpha = opts.alpha;
    let mreach = |i: usize, j: usize, x: &Vec<Vec<f64>>, norms: &Vec<f64>| -> f64 {
        let dij = dist(&x[i], &x[j], metric, norms, i, j) / alpha;
        core[i].max(core[j]).max(dij)
    };

    // -------- exact minimum spanning tree (Prim) ----------------------------
    let mut in_tree = vec![false; n];
    let mut best = vec![f64::INFINITY; n];
    let mut best_src = vec![0usize; n];
    let mut mst: Vec<(usize, usize, f64)> = Vec::with_capacity(n - 1);
    let mut cur = 0usize;
    in_tree[0] = true;
    for _ in 0..n - 1 {
        for j in 0..n {
            if !in_tree[j] {
                let dj = mreach(cur, j, &x, &norms);
                if dj < best[j] {
                    best[j] = dj;
                    best_src[j] = cur;
                }
            }
        }
        let mut pick = usize::MAX;
        let mut pick_d = f64::INFINITY;
        for j in 0..n {
            if !in_tree[j] && best[j] < pick_d {
                pick_d = best[j];
                pick = j;
            }
        }
        if pick == usize::MAX {
            // Only reachable if every remaining distance is non-finite.
            pick = (0..n).find(|&j| !in_tree[j]).unwrap();
            pick_d = f64::MAX;
        }
        mst.push((best_src[pick], pick, pick_d));
        in_tree[pick] = true;
        cur = pick;
    }
    mst.sort_by(|a, b| {
        a.2.partial_cmp(&b.2)
            .unwrap_or(core::cmp::Ordering::Equal)
            .then(a.0.cmp(&b.0))
            .then(a.1.cmp(&b.1))
    });

    // -------- single-linkage hierarchy --------------------------------------
    let mut uf = LinkUf::new(n);
    let mut hierarchy: Vec<(usize, usize, f64, usize)> = Vec::with_capacity(n - 1);
    for &(a, b, delta) in &mst {
        let (aa, bb) = (uf.find(a), uf.find(b));
        hierarchy.push((aa, bb, delta, uf.size[aa] + uf.size[bb]));
        uf.union(aa, bb);
    }

    // -------- condense + stability ------------------------------------------
    let cond = condense(&hierarchy, n, min_cluster_size);
    let (mut stability, max_node) = compute_stability(&cond, n);
    let ctree = cluster_rows(&cond, n);
    let root = n;

    let mut nodes: Vec<usize> = (root..=max_node).collect();
    nodes.sort_unstable_by(|a, b| b.cmp(a)); // descending: children before parents
    let mut is_cluster = vec![false; max_node + 1];
    for &c in &nodes {
        is_cluster[c] = true;
    }

    let mut cluster_size_of = vec![0usize; max_node + 1];
    for r in &ctree {
        cluster_size_of[r.child] = r.size;
    }
    cluster_size_of[root] = ctree
        .iter()
        .filter(|r| r.parent == root)
        .map(|r| r.size)
        .sum::<usize>()
        .max(n);
    let max_cluster_size = if opts.max_cluster_size == 0 {
        n + 1
    } else {
        opts.max_cluster_size as usize
    };

    let eps = opts.cluster_selection_epsilon;
    if selection == "eom" {
        let scan: Vec<usize> = if opts.allow_single_cluster {
            nodes.clone()
        } else {
            nodes.iter().copied().filter(|&c| c != root).collect()
        };
        if !opts.allow_single_cluster {
            is_cluster[root] = false;
        }
        for &node in &scan {
            let subtree: f64 = ctree
                .iter()
                .filter(|r| r.parent == node)
                .map(|r| stability[r.child])
                .sum();
            if subtree > stability[node] || cluster_size_of[node] > max_cluster_size {
                is_cluster[node] = false;
                stability[node] = subtree;
            } else {
                for sub in bfs_condensed(&ctree, n, node) {
                    if sub != node {
                        is_cluster[sub] = false;
                    }
                }
            }
        }
        if eps > 0.0 && !ctree.is_empty() {
            let picked: Vec<usize> = (root..=max_node).filter(|&c| is_cluster[c]).collect();
            let kept = epsilon_search(&picked, &ctree, n, eps, opts.allow_single_cluster, root);
            for c in root..=max_node {
                is_cluster[c] = kept.contains(&c);
            }
        }
    } else {
        let leaves = cluster_tree_leaves(&ctree, root);
        let kept = if eps > 0.0 && !ctree.is_empty() {
            epsilon_search(&leaves, &ctree, n, eps, opts.allow_single_cluster, root)
        } else {
            leaves
        };
        for c in root..=max_node {
            is_cluster[c] = kept.contains(&c);
        }
        if !opts.allow_single_cluster && kept == vec![root] && max_node > root {
            // A lone root selection is the "one big cluster" answer the user
            // opted out of; fall back to the root's immediate children.
            for c in root..=max_node {
                is_cluster[c] = false;
            }
            for r in ctree.iter().filter(|r| r.parent == root) {
                is_cluster[r.child] = true;
            }
        }
    }

    let clusters: Vec<usize> = (root..=max_node).filter(|&c| is_cluster[c]).collect();
    let mut label_of = vec![usize::MAX; max_node + 1];
    for (i, &c) in clusters.iter().enumerate() {
        label_of[c] = i;
    }

    // -------- labels --------------------------------------------------------
    let mut flat = FlatUf::new(max_node + 1);
    for r in &cond {
        if !(r.child >= n && is_cluster[r.child]) {
            flat.union(r.parent, r.child);
        }
    }
    let max_lambda_overall = cond
        .iter()
        .map(|r| r.lambda)
        .filter(|l| l.is_finite())
        .fold(0.0f64, f64::max);
    let root_max_lambda = cond
        .iter()
        .filter(|r| r.parent == root)
        .map(|r| r.lambda)
        .filter(|l| l.is_finite())
        .fold(0.0f64, f64::max);
    let point_lambda: Vec<f64> = {
        let mut v = vec![0.0f64; n];
        for r in &cond {
            if r.child < n {
                v[r.child] = r.lambda;
            }
        }
        v
    };

    let mut labels = vec![-1i64; n];
    for p in 0..n {
        let owner = flat.find(p);
        if owner < root {
            labels[p] = -1;
        } else if owner == root {
            // Points that only ever belonged to the root are noise, unless the
            // user allowed the single-cluster answer and they survive to the
            // deepest density level the root reaches.
            if clusters.len() == 1 && opts.allow_single_cluster && is_cluster[root] {
                let keep = if eps > 0.0 {
                    point_lambda[p] >= 1.0 / eps
                } else {
                    point_lambda[p] >= root_max_lambda
                };
                labels[p] = if keep { label_of[root] as i64 } else { -1 };
            } else {
                labels[p] = -1;
            }
        } else {
            labels[p] = label_of[owner] as i64;
        }
    }

    // -------- probabilities + GLOSH outlier scores --------------------------
    // deaths[c] = the deepest density level any direct child of c reaches.
    let mut deaths = vec![0.0f64; max_node + 1];
    for r in &cond {
        if r.lambda.is_finite() && r.lambda > deaths[r.parent] {
            deaths[r.parent] = r.lambda;
        }
    }
    let mut probability = vec![0.0f64; n];
    let mut outlier = vec![0.0f64; n];
    for r in &cond {
        if r.child >= n {
            continue;
        }
        let p = r.child;
        let parent_death = deaths[r.parent];
        if parent_death > 0.0 && r.lambda.is_finite() {
            outlier[p] = ((parent_death - r.lambda) / parent_death).clamp(0.0, 1.0);
        }
        if labels[p] < 0 {
            continue;
        }
        let owner = clusters[labels[p] as usize];
        let max_lambda = deaths[owner];
        probability[p] = if max_lambda <= 0.0 || !r.lambda.is_finite() {
            1.0
        } else {
            (r.lambda.min(max_lambda) / max_lambda).clamp(0.0, 1.0)
        };
    }

    // -------- per-cluster summary -------------------------------------------
    let mut infos: Vec<ClusterInfo> = Vec::with_capacity(clusters.len());
    for (i, &c) in clusters.iter().enumerate() {
        let members: Vec<usize> = (0..n).filter(|&p| labels[p] == i as i64).collect();
        if members.is_empty() {
            continue;
        }
        let mut centroid = vec![0.0f64; d];
        for &p in &members {
            for kk in 0..d {
                centroid[kk] += raw[p][kk];
            }
        }
        for v in centroid.iter_mut() {
            *v /= members.len() as f64;
        }
        let medoid = *members
            .iter()
            .min_by(|&&a, &&b| {
                let da: f64 = (0..d).map(|kk| (raw[a][kk] - centroid[kk]).powi(2)).sum();
                let db: f64 = (0..d).map(|kk| (raw[b][kk] - centroid[kk]).powi(2)).sum();
                da.partial_cmp(&db)
                    .unwrap_or(core::cmp::Ordering::Equal)
                    .then(a.cmp(&b))
            })
            .unwrap();
        let persistence = if !max_lambda_overall.is_finite()
            || max_lambda_overall == 0.0
            || members.is_empty()
        {
            1.0
        } else {
            (stability[c] / (members.len() as f64 * max_lambda_overall)).clamp(0.0, 1.0)
        };
        infos.push(ClusterInfo {
            label: i,
            size: members.len(),
            persistence,
            centroid,
            medoid_row: scored_rows[medoid] + 1,
        });
    }
    let n_noise = labels.iter().filter(|&&l| l < 0).count();

    // -------- assemble the listed rows --------------------------------------
    let mut listed: Vec<Listed> = Vec::with_capacity(t.rows.len());
    let mut seen = 0usize;
    for ri in 0..t.rows.len() {
        if seen < scored_rows.len() && scored_rows[seen] == ri {
            listed.push(Listed {
                row_no: ri + 1,
                cluster: Some(labels[seen]),
                probability: Some(probability[seen]),
                outlier: Some(outlier[seen]),
            });
            seen += 1;
        } else {
            listed.push(Listed {
                row_no: ri + 1,
                cluster: None,
                probability: None,
                outlier: None,
            });
        }
    }
    match sort {
        "cluster" => listed.sort_by(|a, b| {
            let key = |r: &Listed| match r.cluster {
                Some(c) if c >= 0 => (0i64, c),
                Some(_) => (1, 0),
                None => (2, 0),
            };
            key(a).cmp(&key(b)).then(a.row_no.cmp(&b.row_no))
        }),
        "outlier" => listed.sort_by(|a, b| {
            b.outlier
                .unwrap_or(-1.0)
                .partial_cmp(&a.outlier.unwrap_or(-1.0))
                .unwrap_or(core::cmp::Ordering::Equal)
                .then(a.row_no.cmp(&b.row_no))
        }),
        _ => {}
    }
    if opts.only_noise {
        listed.retain(|r| r.cluster == Some(-1));
    }
    let listed_total = listed.len();
    if opts.top > 0 && listed.len() > opts.top as usize {
        listed.truncate(opts.top as usize);
    }

    // -------- render ---------------------------------------------------------
    let dec = opts.decimals;
    let feat_names: Vec<String> = feats.iter().map(|&c| t.names[c].clone()).collect();
    let min_samples_note = if opts.min_samples == 0 {
        format!("{k} (mirrors min cluster size)")
    } else {
        k.to_string()
    };
    let selection_label = if selection == "eom" {
        "excess of mass"
    } else {
        "leaf"
    };

    match format {
        "csv" => {
            let mut out = String::new();
            let mut head: Vec<String> = t.names.iter().map(|s| csv_cell(s)).collect();
            head.push("cluster".into());
            head.push("probability".into());
            head.push("outlier_score".into());
            out.push_str(&head.join(","));
            out.push('\n');
            for r in &listed {
                let src = &t.rows[r.row_no - 1];
                let mut line: Vec<String> = src.iter().map(|c| csv_cell(c)).collect();
                match (r.cluster, r.probability, r.outlier) {
                    (Some(c), Some(p), Some(o)) => {
                        line.push(c.to_string());
                        line.push(fmt(p, dec));
                        line.push(fmt(o, dec));
                    }
                    _ => {
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
            out.push_str(&format!("  \"metric\": {},\n", json_str(metric.label())));
            out.push_str(&format!("  \"normalized\": {},\n", opts.normalize));
            out.push_str(&format!("  \"min_cluster_size\": {min_cluster_size},\n"));
            out.push_str(&format!("  \"min_samples\": {k},\n"));
            out.push_str(&format!("  \"alpha\": {},\n", fmt(alpha, dec)));
            out.push_str(&format!(
                "  \"cluster_selection_epsilon\": {},\n",
                fmt(eps, dec)
            ));
            out.push_str(&format!("  \"selection\": {},\n", json_str(selection)));
            out.push_str(&format!(
                "  \"allow_single_cluster\": {},\n",
                opts.allow_single_cluster
            ));
            out.push_str(&format!(
                "  \"features\": [{}],\n",
                feat_names
                    .iter()
                    .map(|s| json_str(s))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
            out.push_str(&format!("  \"rows_clustered\": {n},\n"));
            out.push_str(&format!("  \"rows_skipped\": {},\n", t.rows.len() - n));
            out.push_str(&format!("  \"imputed_cells\": {imputed_cells},\n"));
            out.push_str(&format!("  \"clusters_found\": {},\n", infos.len()));
            out.push_str(&format!("  \"noise_rows\": {n_noise},\n"));
            out.push_str(&format!(
                "  \"noise_share\": {},\n",
                fmt(n_noise as f64 / n as f64, dec)
            ));
            out.push_str("  \"clusters\": [\n");
            for (i, c) in infos.iter().enumerate() {
                out.push_str(&format!(
                    "    {{\"cluster\": {}, \"size\": {}, \"persistence\": {}, \"medoid_row\": {}, \"centroid\": {{{}}}}}{}\n",
                    c.label,
                    c.size,
                    fmt(c.persistence, dec),
                    c.medoid_row,
                    feat_names
                        .iter()
                        .zip(&c.centroid)
                        .map(|(nm, v)| format!("{}: {}", json_str(nm), fmt(*v, dec)))
                        .collect::<Vec<_>>()
                        .join(", "),
                    if i + 1 < infos.len() { "," } else { "" }
                ));
            }
            out.push_str("  ],\n");
            out.push_str("  \"rows\": [\n");
            for (i, r) in listed.iter().enumerate() {
                match (r.cluster, r.probability, r.outlier) {
                    (Some(c), Some(p), Some(o)) => out.push_str(&format!(
                        "    {{\"row\": {}, \"cluster\": {}, \"probability\": {}, \"outlier_score\": {}}}{}\n",
                        r.row_no,
                        c,
                        fmt(p, dec),
                        fmt(o, dec),
                        if i + 1 < listed.len() { "," } else { "" }
                    )),
                    _ => out.push_str(&format!(
                        "    {{\"row\": {}, \"cluster\": null, \"probability\": null, \"outlier_score\": null, \"skipped\": true}}{}\n",
                        r.row_no,
                        if i + 1 < listed.len() { "," } else { "" }
                    )),
                }
            }
            out.push_str("  ]\n}");
            Ok(out)
        }
        _ => {
            let mut out = String::new();
            out.push_str(&format!(
                "HDBSCAN — {} cluster{}, {n_noise} noise row{}\n\n",
                infos.len(),
                if infos.len() == 1 { "" } else { "s" },
                if n_noise == 1 { "" } else { "s" }
            ));
            out.push_str(&format!(
                "Rows clustered:   {n} of {} data rows\n",
                t.rows.len()
            ));
            out.push_str(&format!(
                "Features:         {d} of {} columns — {}\n",
                t.names.len(),
                feat_names.join(", ")
            ));
            out.push_str(&format!(
                "Metric:           {}{}\n",
                metric.label(),
                if opts.normalize {
                    " (standardized)"
                } else {
                    " (raw values)"
                }
            ));
            out.push_str(&format!("Min cluster size: {min_cluster_size}\n"));
            out.push_str(&format!("Min samples:      {min_samples_note}\n"));
            out.push_str(&format!("Selection:        {selection_label}\n"));
            if eps > 0.0 {
                out.push_str(&format!(
                    "Merge below:      {} (cluster selection epsilon)\n",
                    fmt(eps, dec)
                ));
            }
            if imputed_cells > 0 {
                out.push_str(&format!(
                    "Imputed cells:    {imputed_cells} ({missing})\n"
                ));
            }
            out.push_str(&format!(
                "Noise:            {n_noise} of {n} rows ({})\n\n",
                pct(n_noise as f64 / n as f64)
            ));

            let mut headers: Vec<String> = vec![
                "Cluster".into(),
                "Size".into(),
                "Persistence".into(),
                "Medoid row".into(),
            ];
            headers.extend(feat_names.iter().map(|f| format!("mean {f}")));
            let mut crows: Vec<Vec<String>> = Vec::new();
            for c in &infos {
                let mut row = vec![
                    c.label.to_string(),
                    c.size.to_string(),
                    fmt(c.persistence, dec),
                    c.medoid_row.to_string(),
                ];
                row.extend(c.centroid.iter().map(|v| fmt(*v, dec)));
                crows.push(row);
            }
            if crows.is_empty() {
                out.push_str("No cluster passed the min cluster size — every row is noise. Lower min cluster size or min samples, or turn on 'Allow single cluster'.\n\n");
            } else {
                out.push_str(&table(&headers, &crows, &[false]));
                out.push('\n');
            }

            let mut rheaders: Vec<String> = vec![
                "Row".into(),
                "Cluster".into(),
                "Probability".into(),
                "Outlier".into(),
            ];
            rheaders.extend(feat_names.iter().cloned());
            let mut rrows: Vec<Vec<String>> = Vec::new();
            for r in &listed {
                let src_idx = scored_rows.iter().position(|&s| s == r.row_no - 1);
                let mut row = vec![r.row_no.to_string()];
                match (r.cluster, r.probability, r.outlier) {
                    (Some(c), Some(p), Some(o)) => {
                        row.push(if c < 0 { "noise".into() } else { c.to_string() });
                        row.push(fmt(p, dec));
                        row.push(fmt(o, dec));
                    }
                    _ => {
                        row.push("skipped".into());
                        row.push("—".into());
                        row.push("—".into());
                    }
                }
                match src_idx {
                    Some(si) => row.extend(raw[si].iter().map(|v| fmt(*v, dec))),
                    None => row.extend(feats.iter().map(|&c| t.rows[r.row_no - 1][c].clone())),
                }
                rrows.push(row);
            }
            if rrows.is_empty() {
                out.push_str("No rows matched the listing filters.\n");
            } else {
                out.push_str(&table(&rheaders, &rrows, &[false]));
            }
            if opts.top > 0 && listed_total > listed.len() {
                out.push_str(&format!(
                    "\n… {} more listed row{} hidden by Top N.\n",
                    listed_total - listed.len(),
                    if listed_total - listed.len() == 1 {
                        ""
                    } else {
                        "s"
                    }
                ));
            }
            Ok(out)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two well-separated blobs of 6 points each plus one far-away stray.
    const TWO_BLOBS: &str = "x,y\n1,1\n1,2\n2,1\n2,2\n1.5,1.5\n1.2,1.8\n\
                             10,10\n10,11\n11,10\n11,11\n10.5,10.5\n10.2,10.8\n\
                             60,-40";

    fn opts() -> Options {
        Options::default()
    }

    #[test]
    fn finds_two_blobs_and_marks_the_stray_as_noise() {
        let o = Options {
            format: "json".into(),
            ..opts()
        };
        let out = run(TWO_BLOBS, &o).unwrap();
        assert!(
            out.contains("\"clusters_found\": 2"),
            "expected 2 clusters, got:\n{out}"
        );
        assert!(
            out.contains("\"noise_rows\": 1"),
            "expected 1 noise row, got:\n{out}"
        );
        // Row 13 is the stray and must be the noise row.
        assert!(
            out.contains(r#"{"row": 13, "cluster": -1"#),
            "row 13 should be noise, got:\n{out}"
        );
    }

    #[test]
    fn noise_rows_get_zero_probability_and_members_get_positive() {
        let out = run(
            TWO_BLOBS,
            &Options {
                format: "csv".into(),
                ..opts()
            },
        )
        .unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[0], "x,y,cluster,probability,outlier_score");
        let stray = lines.last().unwrap();
        assert!(stray.starts_with("60,-40,-1,0.0000,"), "got {stray}");
        // First blob member sits in a cluster with a non-zero probability.
        let first = lines[1];
        let cols: Vec<&str> = first.split(',').collect();
        assert_ne!(cols[2], "-1", "row 1 should be clustered: {first}");
        assert!(cols[3].parse::<f64>().unwrap() > 0.0, "got {first}");
    }

    #[test]
    fn min_cluster_size_merges_the_two_blobs_into_noise_or_one_group() {
        // A min cluster size larger than either blob cannot keep both.
        let out = run(
            TWO_BLOBS,
            &Options {
                min_cluster_size: 7,
                format: "json".into(),
                ..opts()
            },
        )
        .unwrap();
        assert!(
            out.contains("\"clusters_found\": 0") || out.contains("\"clusters_found\": 1"),
            "min_cluster_size 7 should not yield two 6-point blobs:\n{out}"
        );
    }

    #[test]
    fn leaf_selection_is_accepted_and_reported() {
        let out = run(
            TWO_BLOBS,
            &Options {
                selection: "leaf".into(),
                ..opts()
            },
        )
        .unwrap();
        assert!(out.contains("Selection:        leaf"), "got:\n{out}");
    }

    #[test]
    fn allow_single_cluster_can_report_one_group() {
        let one_blob = "x,y\n1,1\n1,2\n2,1\n2,2\n1.5,1.5\n1.2,1.8\n1.4,1.1";
        let out = run(
            one_blob,
            &Options {
                allow_single_cluster: true,
                format: "json".into(),
                ..opts()
            },
        )
        .unwrap();
        assert!(
            out.contains("\"allow_single_cluster\": true"),
            "got:\n{out}"
        );
    }

    #[test]
    fn every_metric_runs() {
        for m in ["euclidean", "manhattan", "chebyshev", "cosine"] {
            let out = run(
                TWO_BLOBS,
                &Options {
                    metric: m.into(),
                    ..opts()
                },
            );
            assert!(out.is_ok(), "metric {m} failed: {:?}", out.err());
        }
    }

    #[test]
    fn cluster_selection_epsilon_does_not_increase_the_cluster_count() {
        let base = run(
            TWO_BLOBS,
            &Options {
                format: "json".into(),
                ..opts()
            },
        )
        .unwrap();
        let merged = run(
            TWO_BLOBS,
            &Options {
                cluster_selection_epsilon: 5.0,
                format: "json".into(),
                ..opts()
            },
        )
        .unwrap();
        let count = |s: &str| -> usize {
            s.lines()
                .find(|l| l.contains("\"clusters_found\""))
                .and_then(|l| l.split(':').nth(1))
                .map(|v| v.trim().trim_end_matches(',').parse().unwrap())
                .unwrap()
        };
        assert!(
            count(&merged) <= count(&base),
            "epsilon merging should not add clusters ({} -> {})",
            count(&base),
            count(&merged)
        );
    }

    #[test]
    fn only_noise_and_top_filter_the_listing() {
        let out = run(
            TWO_BLOBS,
            &Options {
                only_noise: true,
                format: "csv".into(),
                ..opts()
            },
        )
        .unwrap();
        // Header + exactly the one noise row.
        assert_eq!(out.lines().count(), 2, "got:\n{out}");
        assert!(out.lines().nth(1).unwrap().starts_with("60,-40,-1"));

        let top = run(
            TWO_BLOBS,
            &Options {
                top: 3,
                sort: "outlier".into(),
                format: "csv".into(),
                ..opts()
            },
        )
        .unwrap();
        assert_eq!(top.lines().count(), 4, "header + 3 rows, got:\n{top}");
    }

    #[test]
    fn missing_median_imputes_instead_of_dropping() {
        let data = "x,y\n1,1\n1,2\n2,1\n2,\n1.5,1.5\n1.2,1.8\n10,10\n10,11\n11,10\n11,11\n10.5,10.5\n10.2,10.8";
        let dropped = run(
            data,
            &Options {
                format: "json".into(),
                ..opts()
            },
        )
        .unwrap();
        assert!(dropped.contains("\"rows_skipped\": 1"), "got:\n{dropped}");
        let filled = run(
            data,
            &Options {
                missing: "median".into(),
                format: "json".into(),
                ..opts()
            },
        )
        .unwrap();
        assert!(filled.contains("\"rows_skipped\": 0"), "got:\n{filled}");
        assert!(filled.contains("\"imputed_cells\": 1"), "got:\n{filled}");
    }

    #[test]
    fn delimiters_and_header_modes_parse() {
        let out = run(
            "1;1\n1;2\n2;1\n2;2\n1.5;1.5\n1.2;1.8\n10;10\n10;11\n11;10\n11;11\n10.5;10.5\n10.2;10.8",
            &Options {
                delimiter: "semicolon".into(),
                header: "no".into(),
                ..opts()
            },
        )
        .unwrap();
        assert!(out.contains("c1, c2"), "got:\n{out}");
    }

    #[test]
    fn output_is_deterministic() {
        let a = run(TWO_BLOBS, &opts()).unwrap();
        let b = run(TWO_BLOBS, &opts()).unwrap();
        assert_eq!(a, b);
    }

    // ------------------------------------------------------------- errors ---

    #[test]
    fn empty_input_is_an_error() {
        let err = run("   \n  ", &opts()).unwrap_err();
        assert!(err.contains("no data"), "got {err}");
    }

    #[test]
    fn too_few_rows_is_an_error_that_names_the_requirement() {
        let err = run("x,y\n1,1\n2,2", &opts()).unwrap_err();
        assert!(
            err.contains("min_cluster_size") && err.contains("found 2"),
            "got {err}"
        );
    }

    #[test]
    fn non_numeric_columns_are_rejected_with_a_hint() {
        let err = run("name\na\nb\nc\nd\ne\nf", &opts()).unwrap_err();
        assert!(err.contains("no numeric columns"), "got {err}");
    }

    #[test]
    fn ragged_rows_name_the_offending_line() {
        let err = run("x,y\n1,1\n2\n3,3\n4,4\n5,5\n6,6", &opts()).unwrap_err();
        assert!(err.contains("row 3 has 1 columns"), "got {err}");
    }

    #[test]
    fn bad_enum_values_are_rejected() {
        assert!(run(TWO_BLOBS, &Options { metric: "hamming".into(), ..opts() })
            .unwrap_err()
            .contains("metric must be"));
        assert!(run(TWO_BLOBS, &Options { selection: "tree".into(), ..opts() })
            .unwrap_err()
            .contains("selection must be"));
        assert!(run(TWO_BLOBS, &Options { format: "yaml".into(), ..opts() })
            .unwrap_err()
            .contains("format must be"));
        assert!(run(TWO_BLOBS, &Options { sort: "size".into(), ..opts() })
            .unwrap_err()
            .contains("sort must be"));
        assert!(run(TWO_BLOBS, &Options { missing: "skip".into(), ..opts() })
            .unwrap_err()
            .contains("missing must be"));
    }

    #[test]
    fn missing_error_mode_names_the_cell() {
        let data = "x,y\n1,1\n1,2\n2,1\n2,oops\n1.5,1.5\n1.2,1.8";
        // `y` is named explicitly: blank feature selection would skip the whole
        // column as non-numeric and the bad cell would never be reached.
        let err = run(
            data,
            &Options {
                features: "x,y".into(),
                missing: "error".into(),
                ..opts()
            },
        )
        .unwrap_err();
        assert!(err.contains("row 4 column 'y'"), "got {err}");
    }

    #[test]
    fn invalid_numeric_options_are_rejected() {
        assert!(run(TWO_BLOBS, &Options { min_cluster_size: 1, ..opts() })
            .unwrap_err()
            .contains("min_cluster_size must be at least 2"));
        assert!(run(TWO_BLOBS, &Options { alpha: 0.0, ..opts() })
            .unwrap_err()
            .contains("alpha must be"));
        assert!(run(
            TWO_BLOBS,
            &Options {
                cluster_selection_epsilon: -1.0,
                ..opts()
            }
        )
        .unwrap_err()
        .contains("cluster_selection_epsilon must be"));
        assert!(run(TWO_BLOBS, &Options { decimals: 13, ..opts() })
            .unwrap_err()
            .contains("decimals must be"));
    }

    #[test]
    fn row_cap_is_enforced_at_the_exact_boundary() {
        let mut over = String::from("x,y\n");
        for i in 0..=MAX_ROWS {
            over.push_str(&format!("{i},{i}\n"));
        }
        let err = run(&over, &opts()).unwrap_err();
        assert!(
            err.contains(&format!("too many rows: {}", MAX_ROWS + 1)),
            "got {err}"
        );
    }

    #[test]
    fn column_cap_is_enforced() {
        let wide: String = (0..=MAX_COLS)
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let err = run(&format!("{wide}\n{wide}"), &opts()).unwrap_err();
        assert!(err.contains("too many columns: 201"), "got {err}");
    }
}
