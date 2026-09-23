//! pagerank-ranker core — pure compute, shared by the chat skill block and the
//! web page. No wafer/wasm-bindgen deps.
//!
//! Computes PageRank scores for the nodes of a user-supplied graph by power
//! iteration, and ranks the nodes by score. Input is an edge list (the same
//! paste format the `graph-algorithms` block accepts) or a square adjacency
//! matrix. Output is a ranked table as a readable report, JSON, or CSV.
//!
//! The iteration is the standard formulation
//!
//! ```text
//! x_{k+1} = d * (A^T x_k + dangling_mass * v) + (1 - d) * v
//! ```
//!
//! where `d` is the damping factor, `A` is the row-normalised (weighted)
//! adjacency matrix, and `v` is the teleport distribution — uniform unless a
//! personalization vector is supplied.

use std::collections::BTreeMap;

/// Maximum edges accepted from one paste. Stated on the page.
pub const MAX_EDGES: usize = 20_000;
/// Maximum distinct nodes accepted from one paste. Stated on the page.
pub const MAX_NODES: usize = 5_000;

/// Every knob the chat schema, the CLI, and the page share.
#[derive(Clone, Debug)]
pub struct Options {
    /// `auto` | `edge-list` | `matrix`.
    pub input_format: String,
    /// Treat each edge as one-way (`true`) or as a link in both directions.
    pub directed: bool,
    /// Read a per-edge weight (trailing number / matrix cell value).
    pub weighted: bool,
    /// Damping factor `d` — the probability of following a link.
    pub damping: f64,
    /// Hard cap on power-iteration steps.
    pub max_iter: u32,
    /// Convergence threshold on the total (L1) change between iterations.
    pub tolerance: f64,
    /// `redistribute` | `self-loop` | `drop`.
    pub dangling: String,
    /// Optional `node:weight` teleport bias, comma- or newline-separated.
    pub personalization: String,
    /// Keep only the top N rows (0 = all).
    pub top: u32,
    /// Decimal places for scores.
    pub decimals: u32,
    /// `text` | `json` | `csv`.
    pub format: String,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            input_format: "auto".into(),
            directed: true,
            weighted: false,
            damping: 0.85,
            max_iter: 100,
            tolerance: 0.000_001,
            dangling: "redistribute".into(),
            personalization: String::new(),
            top: 0,
            decimals: 6,
            format: "text".into(),
        }
    }
}

/// One node's finished result.
#[derive(Clone, Debug)]
pub struct Ranked {
    pub rank: usize,
    pub node: String,
    pub score: f64,
    /// Percentage of the total PageRank mass.
    pub share: f64,
    pub in_degree: usize,
    pub out_degree: usize,
    pub in_weight: f64,
    pub out_weight: f64,
    pub dangling: bool,
}

/// The full computation, before formatting.
#[derive(Clone, Debug)]
pub struct Report {
    pub ranking: Vec<Ranked>,
    pub node_count: usize,
    pub edge_count: usize,
    pub directed: bool,
    pub weighted: bool,
    pub damping: f64,
    pub tolerance: f64,
    pub max_iter: u32,
    pub iterations: u32,
    pub converged: bool,
    pub final_delta: f64,
    pub dangling_policy: String,
    pub dangling_nodes: Vec<String>,
    pub personalized: bool,
    pub total_mass: f64,
    pub source_format: &'static str,
    /// Rows hidden by `top`.
    pub truncated: usize,
    /// Decimal places the renderers should use.
    pub decimals: usize,
}

/// Parsed graph, indexed by position in `nodes`.
struct Graph {
    nodes: Vec<String>,
    /// `out[i]` = (target index, weight); parallel edges are merged by summing.
    out: Vec<Vec<(usize, f64)>>,
    /// Distinct edges after merging parallel duplicates.
    edge_count: usize,
    source_format: &'static str,
}

fn index_of(nodes: &mut Vec<String>, idx: &mut BTreeMap<String, usize>, label: &str) -> usize {
    if let Some(i) = idx.get(label) {
        return *i;
    }
    let i = nodes.len();
    nodes.push(label.to_string());
    idx.insert(label.to_string(), i);
    i
}

fn is_number(tok: &str) -> bool {
    tok.parse::<f64>().map(|v| v.is_finite()).unwrap_or(false)
}

/// Content lines: blank lines and `#` comments dropped, paired with their
/// 1-based source line number for error messages.
fn content_lines(input: &str) -> Vec<(usize, &str)> {
    input
        .lines()
        .enumerate()
        .map(|(i, l)| (i + 1, l.trim()))
        .filter(|(_, l)| !l.is_empty() && !l.starts_with('#'))
        .collect()
}

/// Split a matrix row on commas, semicolons, pipes, or whitespace.
fn matrix_tokens(line: &str) -> Vec<&str> {
    line.split(|c: char| c == ',' || c == ';' || c == '|' || c.is_whitespace())
        .filter(|t| !t.is_empty())
        .collect()
}

/// A paste auto-detects as a matrix only when it is unambiguous: at least two
/// rows, every row the same length, every token numeric, and the row count
/// equal to the column count. Labelled matrices need `input_format = matrix`.
fn looks_like_matrix(lines: &[(usize, &str)]) -> bool {
    if lines.len() < 2 {
        return false;
    }
    let rows: Vec<Vec<&str>> = lines.iter().map(|(_, l)| matrix_tokens(l)).collect();
    let width = rows[0].len();
    width == rows.len()
        && rows
            .iter()
            .all(|r| r.len() == width && r.iter().all(|t| is_number(t)))
}

/// Parse an edge list. Accepted per line:
///   `a -> b`, `a - b`, `a -- b`, `a b`, `a,b`   an edge
///   `a -> b : 3`, `a,b,3`                       a weighted edge
///   `a`                                         an isolated node
/// Blank lines and `#` comments are ignored. Parallel edges sum their weights.
fn parse_edge_list(input: &str, directed: bool, weighted: bool) -> Result<Graph, String> {
    let lines = content_lines(input);
    if lines.is_empty() {
        return Err("no graph: provide at least one edge, e.g. `a -> b` (one per line)".into());
    }
    let mut nodes: Vec<String> = Vec::new();
    let mut idx: BTreeMap<String, usize> = BTreeMap::new();
    // (source, target) -> summed weight, so parallel links add authority.
    let mut edges: BTreeMap<(usize, usize), f64> = BTreeMap::new();
    let mut raw_edges = 0usize;

    for (lineno, line) in lines {
        // Only a *spaced* hyphen and the arrow/double-dash glyphs separate
        // labels, so a hyphen inside a label or a negative number survives to
        // be reported as an error rather than silently split.
        let norm = line
            .replace("->", " ")
            .replace(" -- ", " ")
            .replace(" - ", " ")
            .replace(',', " ")
            .replace(';', " ")
            .replace('|', " ")
            .replace(':', " ");
        let toks: Vec<&str> = norm.split_whitespace().collect();
        if toks.is_empty() {
            continue;
        }
        if toks.len() == 1 {
            if nodes.len() >= MAX_NODES && !idx.contains_key(toks[0]) {
                return Err(format!("too many nodes: the limit is {MAX_NODES}"));
            }
            index_of(&mut nodes, &mut idx, toks[0]);
            continue;
        }

        let mut weight = 1.0f64;
        if toks.len() >= 3 {
            let last = toks[toks.len() - 1];
            if is_number(last) {
                let parsed: f64 = last.parse().unwrap();
                if parsed < 0.0 {
                    return Err(format!(
                        "line {lineno}: negative weight {parsed} is not allowed — PageRank needs non-negative edge weights"
                    ));
                }
                if weighted {
                    weight = parsed;
                }
            } else {
                return Err(format!(
                    "line {lineno}: expected `source target` or `source target weight`, got {} values ({line:?})",
                    toks.len()
                ));
            }
        }

        raw_edges += 1;
        if raw_edges > MAX_EDGES {
            return Err(format!("too many edges: the limit is {MAX_EDGES} per run"));
        }
        for label in [toks[0], toks[1]] {
            if nodes.len() >= MAX_NODES && !idx.contains_key(label) {
                return Err(format!("too many nodes: the limit is {MAX_NODES}"));
            }
            index_of(&mut nodes, &mut idx, label);
        }
        let a = idx[toks[0]];
        let b = idx[toks[1]];
        *edges.entry((a, b)).or_insert(0.0) += weight;
        if !directed && a != b {
            *edges.entry((b, a)).or_insert(0.0) += weight;
        }
    }

    if nodes.is_empty() {
        return Err("no graph: provide at least one edge, e.g. `a -> b` (one per line)".into());
    }
    Ok(build_graph(nodes, edges, "edge list"))
}

/// Parse a square adjacency matrix. `matrix[i][j] > 0` means an edge i → j
/// (row = source). An optional header row and/or leading column of labels name
/// the nodes; otherwise nodes are numbered `1`..`n`.
fn parse_matrix(input: &str, directed: bool, weighted: bool) -> Result<Graph, String> {
    let lines = content_lines(input);
    if lines.is_empty() {
        return Err("no matrix: provide one row of numbers per node".into());
    }
    let mut rows: Vec<(usize, Vec<String>)> = lines
        .iter()
        .map(|(n, l)| (*n, matrix_tokens(l).into_iter().map(String::from).collect()))
        .collect();

    // A first row with no numeric tokens is a header of node labels.
    let header: Option<Vec<String>> = if rows[0].1.iter().all(|t| !is_number(t)) {
        Some(rows.remove(0).1)
    } else {
        None
    };
    if rows.is_empty() {
        return Err("no matrix rows after the header: provide one row of numbers per node".into());
    }
    // A leading non-numeric token on every row is that row's label.
    let row_labels: Option<Vec<String>> = if rows
        .iter()
        .all(|(_, r)| r.first().map(|t| !is_number(t)).unwrap_or(false))
    {
        Some(
            rows.iter_mut()
                .map(|(_, r)| r.remove(0))
                .collect::<Vec<String>>(),
        )
    } else {
        None
    };

    let n = rows.len();
    if n > MAX_NODES {
        return Err(format!("too many nodes: the limit is {MAX_NODES}"));
    }
    for (lineno, row) in &rows {
        if row.len() != n {
            return Err(format!(
                "line {lineno}: adjacency matrix must be square — expected {n} values (one per row), got {}",
                row.len()
            ));
        }
    }

    let labels: Vec<String> = match (row_labels, header) {
        (Some(l), _) => l,
        (None, Some(h)) if h.len() == n => h,
        // A header with a leading corner cell (`  a b c`) is one longer.
        (None, Some(h)) if h.len() == n + 1 => h[1..].to_vec(),
        (None, Some(h)) => {
            return Err(format!(
                "header names {} nodes but the matrix has {n} rows",
                h.len()
            ))
        }
        (None, None) => (1..=n).map(|i| i.to_string()).collect(),
    };
    let mut seen: BTreeMap<&str, usize> = BTreeMap::new();
    for label in &labels {
        if seen.insert(label.as_str(), 0).is_some() {
            return Err(format!("duplicate node label {label:?} in the matrix"));
        }
    }

    let mut edges: BTreeMap<(usize, usize), f64> = BTreeMap::new();
    for (i, (lineno, row)) in rows.iter().enumerate() {
        for (j, cell) in row.iter().enumerate() {
            let v: f64 = cell.parse().map_err(|_| {
                format!("line {lineno}: {cell:?} is not a number — adjacency cells must be numeric")
            })?;
            if !v.is_finite() || v < 0.0 {
                return Err(format!(
                    "line {lineno}: cell value {v} is not allowed — adjacency cells must be zero or a positive weight"
                ));
            }
            if v == 0.0 {
                continue;
            }
            let w = if weighted { v } else { 1.0 };
            *edges.entry((i, j)).or_insert(0.0) += w;
            if !directed && i != j {
                *edges.entry((j, i)).or_insert(0.0) += w;
            }
            if edges.len() > MAX_EDGES {
                return Err(format!("too many edges: the limit is {MAX_EDGES} per run"));
            }
        }
    }
    Ok(build_graph(labels, edges, "adjacency matrix"))
}

fn build_graph(
    nodes: Vec<String>,
    edges: BTreeMap<(usize, usize), f64>,
    source_format: &'static str,
) -> Graph {
    let mut out: Vec<Vec<(usize, f64)>> = vec![Vec::new(); nodes.len()];
    for ((a, b), w) in &edges {
        out[*a].push((*b, *w));
    }
    Graph {
        nodes,
        out,
        edge_count: edges.len(),
        source_format,
    }
}

/// Parse `a:0.5, b:0.5` (also `a=0.5` or `a 0.5`, comma- or newline-separated)
/// into a normalised teleport vector over the graph's nodes.
fn parse_personalization(spec: &str, g: &Graph) -> Result<Vec<f64>, String> {
    let mut v = vec![0.0f64; g.nodes.len()];
    let index: BTreeMap<&str, usize> = g
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.as_str(), i))
        .collect();
    let mut any = false;
    for entry in spec
        .split(|c: char| c == ',' || c == '\n' || c == ';')
        .map(str::trim)
        .filter(|e| !e.is_empty())
    {
        let cleaned = entry.replace(':', " ").replace('=', " ");
        let toks: Vec<&str> = cleaned.split_whitespace().collect();
        if toks.len() != 2 {
            return Err(format!(
                "personalization entry {entry:?} is not `node:weight` — write e.g. `a:0.7, b:0.3`"
            ));
        }
        let i = *index
            .get(toks[0])
            .ok_or_else(|| format!("personalization node {:?} is not in the graph", toks[0]))?;
        let w: f64 = toks[1].parse().map_err(|_| {
            format!(
                "personalization weight {:?} for node {:?} is not a number",
                toks[1], toks[0]
            )
        })?;
        if !w.is_finite() || w < 0.0 {
            return Err(format!(
                "personalization weight {w} for node {:?} must be zero or positive",
                toks[0]
            ));
        }
        v[i] += w;
        any = true;
    }
    if !any {
        return Err("personalization is empty — give at least one `node:weight`".into());
    }
    let total: f64 = v.iter().sum();
    if total <= 0.0 {
        return Err("personalization weights sum to zero — at least one must be positive".into());
    }
    for x in v.iter_mut() {
        *x /= total;
    }
    Ok(v)
}

/// Parse + rank. The formatting-free half, so tests can assert on numbers.
pub fn compute(input: &str, opts: &Options) -> Result<Report, String> {
    let format = opts.input_format.trim();
    let g = match format {
        "" | "auto" => {
            if looks_like_matrix(&content_lines(input)) {
                parse_matrix(input, opts.directed, opts.weighted)?
            } else {
                parse_edge_list(input, opts.directed, opts.weighted)?
            }
        }
        "edge-list" => parse_edge_list(input, opts.directed, opts.weighted)?,
        "matrix" => parse_matrix(input, opts.directed, opts.weighted)?,
        other => {
            return Err(format!(
                "invalid input_format {other:?}: expected auto, edge-list, or matrix"
            ))
        }
    };
    if !(0.0..=0.999).contains(&opts.damping) || !opts.damping.is_finite() {
        return Err(format!(
            "damping must be between 0 and 0.999, got {}",
            opts.damping
        ));
    }
    if opts.max_iter == 0 {
        return Err("max_iter must be at least 1".into());
    }
    if !opts.tolerance.is_finite() || opts.tolerance <= 0.0 {
        return Err(format!(
            "tolerance must be a positive number, got {}",
            opts.tolerance
        ));
    }
    let policy = opts.dangling.trim();
    if !matches!(policy, "" | "redistribute" | "self-loop" | "drop") {
        return Err(format!(
            "invalid dangling {policy:?}: expected redistribute, self-loop, or drop"
        ));
    }
    let policy = if policy.is_empty() {
        "redistribute"
    } else {
        policy
    };

    let n = g.nodes.len();
    let personalized = !opts.personalization.trim().is_empty();
    let teleport = if personalized {
        parse_personalization(&opts.personalization, &g)?
    } else {
        vec![1.0 / n as f64; n]
    };

    let out_weight: Vec<f64> = g
        .out
        .iter()
        .map(|list| list.iter().map(|(_, w)| *w).sum())
        .collect();
    // A zero-weight row (every weight 0) is dangling too, not a divide-by-zero.
    let is_dangling: Vec<bool> = out_weight.iter().map(|w| *w <= 0.0).collect();

    let mut x = vec![1.0 / n as f64; n];
    let mut next = vec![0.0f64; n];
    let mut iterations = 0u32;
    let mut converged = false;
    let mut final_delta = f64::INFINITY;

    for step in 1..=opts.max_iter {
        iterations = step;
        next.iter_mut().for_each(|v| *v = 0.0);
        let mut dangling_mass = 0.0;
        for i in 0..n {
            if is_dangling[i] {
                match policy {
                    "self-loop" => next[i] += x[i],
                    "drop" => {}
                    _ => dangling_mass += x[i],
                }
                continue;
            }
            let share = x[i] / out_weight[i];
            for (j, w) in &g.out[i] {
                next[*j] += share * w;
            }
        }
        let mut delta = 0.0;
        for j in 0..n {
            let v = opts.damping * (next[j] + dangling_mass * teleport[j])
                + (1.0 - opts.damping) * teleport[j];
            delta += (v - x[j]).abs();
            next[j] = v;
        }
        std::mem::swap(&mut x, &mut next);
        final_delta = delta;
        if delta < opts.tolerance {
            converged = true;
            break;
        }
    }

    let mut in_degree = vec![0usize; n];
    let mut in_weight = vec![0.0f64; n];
    for i in 0..n {
        for (j, w) in &g.out[i] {
            in_degree[*j] += 1;
            in_weight[*j] += w;
        }
    }

    let total_mass: f64 = x.iter().sum();
    let mut order: Vec<usize> = (0..n).collect();
    // Score descending; ties keep first-seen input order so output is stable.
    order.sort_by(|a, b| {
        x[*b]
            .partial_cmp(&x[*a])
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.cmp(b))
    });

    let mut ranking: Vec<Ranked> = order
        .iter()
        .enumerate()
        .map(|(pos, &i)| Ranked {
            rank: pos + 1,
            node: g.nodes[i].clone(),
            score: x[i],
            share: if total_mass > 0.0 {
                x[i] / total_mass * 100.0
            } else {
                0.0
            },
            in_degree: in_degree[i],
            out_degree: g.out[i].len(),
            in_weight: in_weight[i],
            out_weight: out_weight[i],
            dangling: is_dangling[i],
        })
        .collect();

    let truncated = if opts.top > 0 && (opts.top as usize) < ranking.len() {
        let hidden = ranking.len() - opts.top as usize;
        ranking.truncate(opts.top as usize);
        hidden
    } else {
        0
    };

    let dangling_nodes: Vec<String> = (0..n)
        .filter(|i| is_dangling[*i])
        .map(|i| g.nodes[i].clone())
        .collect();

    Ok(Report {
        ranking,
        node_count: n,
        edge_count: g.edge_count,
        directed: opts.directed,
        weighted: opts.weighted,
        damping: opts.damping,
        tolerance: opts.tolerance,
        max_iter: opts.max_iter,
        iterations,
        converged,
        final_delta,
        dangling_policy: policy.to_string(),
        dangling_nodes,
        personalized,
        total_mass,
        source_format: g.source_format,
        truncated,
        decimals: opts.decimals.min(12) as usize,
    })
}

fn round_to(v: f64, decimals: u32) -> String {
    let epsilon = 0.5 * 10f64.powi(-(decimals as i32));
    let v = if v.abs() < epsilon { 0.0 } else { v };
    format!("{:.*}", decimals as usize, v)
}

fn json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
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

/// Render a setting (damping, tolerance) the way the user typed it.
fn compact(v: f64) -> String {
    if v == 0.0 {
        return "0".into();
    }
    if v.abs() >= 1e-9 && v.abs() < 1e9 {
        format!("{v}")
    } else {
        format!("{v:.3e}")
    }
}

/// Convergence deltas are tiny; scientific notation keeps them readable.
fn sci(v: f64) -> String {
    format!("{v:.3e}")
}

fn render_text(r: &Report) -> String {
    let d = r.decimals;
    let mut out = String::new();
    out.push_str(&format!(
        "PageRank — {} node{}, {} edge{} ({}, {})\n",
        r.node_count,
        if r.node_count == 1 { "" } else { "s" },
        r.edge_count,
        if r.edge_count == 1 { "" } else { "s" },
        if r.directed { "directed" } else { "undirected" },
        if r.weighted { "weighted" } else { "unweighted" },
    ));
    out.push_str(&format!(
        "Source: {} · damping {} · tolerance {} · max {} iteration{} · dangling: {}\n",
        r.source_format,
        compact(r.damping),
        compact(r.tolerance),
        r.max_iter,
        if r.max_iter == 1 { "" } else { "s" },
        r.dangling_policy,
    ));
    if r.personalized {
        out.push_str("Teleport: personalized (the random jump follows your weights)\n");
    }
    if r.converged {
        out.push_str(&format!(
            "Converged after {} iteration{} (final change {})\n",
            r.iterations,
            if r.iterations == 1 { "" } else { "s" },
            sci(r.final_delta)
        ));
    } else {
        out.push_str(&format!(
            "NOT CONVERGED: hit the {}-iteration cap with a final change of {} (tolerance {}). The scores below are the last iterate — raise max_iter or tolerance.\n",
            r.max_iter,
            sci(r.final_delta),
            compact(r.tolerance)
        ));
    }
    out.push('\n');

    let node_w = r
        .ranking
        .iter()
        .map(|x| x.node.chars().count())
        .max()
        .unwrap_or(4)
        .max(4);
    let score_w = (d + 2).max(8);
    out.push_str(&format!(
        "{:>4}  {:<node_w$}  {:>score_w$}  {:>7}  {:>4}  {:>4}\n",
        "Rank", "Node", "PageRank", "Share", "In", "Out"
    ));
    for row in &r.ranking {
        out.push_str(&format!(
            "{:>4}  {:<node_w$}  {:>score_w$}  {:>6}%  {:>4}  {:>4}{}\n",
            row.rank,
            row.node,
            round_to(row.score, d as u32),
            format!("{:.2}", row.share),
            row.in_degree,
            row.out_degree,
            if row.dangling { "  (dangling)" } else { "" },
        ));
    }
    if r.truncated > 0 {
        out.push_str(&format!(
            "… {} lower-ranked node{} hidden by top={}\n",
            r.truncated,
            if r.truncated == 1 { "" } else { "s" },
            r.ranking.len()
        ));
    }
    out.push_str(&format!(
        "\nTotal PageRank mass: {}\n",
        round_to(r.total_mass, d as u32)
    ));
    if !r.dangling_nodes.is_empty() {
        out.push_str(&format!(
            "Dangling nodes (no outgoing links): {}\n",
            r.dangling_nodes.join(", ")
        ));
    }
    out
}

fn render_json(r: &Report) -> String {
    let d = r.decimals as u32;
    let mut out = String::from("{\n");
    out.push_str(&format!("  \"nodes\": {},\n", r.node_count));
    out.push_str(&format!("  \"edges\": {},\n", r.edge_count));
    out.push_str(&format!(
        "  \"source_format\": {},\n",
        json_str(r.source_format)
    ));
    out.push_str(&format!("  \"directed\": {},\n", r.directed));
    out.push_str(&format!("  \"weighted\": {},\n", r.weighted));
    out.push_str(&format!("  \"damping\": {},\n", r.damping));
    out.push_str(&format!("  \"tolerance\": {},\n", r.tolerance));
    out.push_str(&format!("  \"max_iterations\": {},\n", r.max_iter));
    out.push_str(&format!("  \"iterations\": {},\n", r.iterations));
    out.push_str(&format!("  \"converged\": {},\n", r.converged));

    out.push_str(&format!(
        "  \"dangling_policy\": {},\n",
        json_str(&r.dangling_policy)
    ));
    out.push_str(&format!(
        "  \"dangling_nodes\": [{}],\n",
        r.dangling_nodes
            .iter()
            .map(|n| json_str(n))
            .collect::<Vec<_>>()
            .join(", ")
    ));
    out.push_str(&format!("  \"personalized\": {},\n", r.personalized));
    out.push_str(&format!(
        "  \"total_mass\": {},\n",
        round_to(r.total_mass, d)
    ));
    out.push_str(&format!("  \"hidden_by_top\": {},\n", r.truncated));
    out.push_str("  \"ranking\": [\n");
    for (i, row) in r.ranking.iter().enumerate() {
        out.push_str(&format!(
            "    {{ \"rank\": {}, \"node\": {}, \"score\": {}, \"share_percent\": {}, \"in_degree\": {}, \"out_degree\": {}, \"in_weight\": {}, \"out_weight\": {}, \"dangling\": {} }}{}\n",
            row.rank,
            json_str(&row.node),
            round_to(row.score, d),
            format!("{:.2}", row.share),
            row.in_degree,
            row.out_degree,
            round_to(row.in_weight, d),
            round_to(row.out_weight, d),
            row.dangling,
            if i + 1 == r.ranking.len() { "" } else { "," }
        ));
    }
    out.push_str("  ]\n}\n");
    out
}

fn render_csv(r: &Report) -> String {
    let d = r.decimals as u32;
    let mut out = String::from(
        "rank,node,pagerank,share_percent,in_degree,out_degree,in_weight,out_weight,dangling\n",
    );
    for row in &r.ranking {
        out.push_str(&format!(
            "{},{},{},{:.2},{},{},{},{},{}\n",
            row.rank,
            csv_cell(&row.node),
            round_to(row.score, d),
            row.share,
            row.in_degree,
            row.out_degree,
            round_to(row.in_weight, d),
            round_to(row.out_weight, d),
            row.dangling,
        ));
    }
    out
}

/// Compute PageRank and render it in the requested format.
pub fn run(input: &str, opts: &Options) -> Result<String, String> {
    if opts.decimals > 12 {
        return Err(format!(
            "decimals must be between 0 and 12, got {}",
            opts.decimals
        ));
    }
    let mut report = compute(input, opts)?;
    report.decimals = opts.decimals as usize;
    match opts.format.trim() {
        "" | "text" => Ok(render_text(&report)),
        "json" => Ok(render_json(&report)),
        "csv" => Ok(render_csv(&report)),
        other => Err(format!(
            "invalid format {other:?}: expected text, json, or csv"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> Options {
        Options::default()
    }

    fn scores(input: &str, o: &Options) -> BTreeMap<String, f64> {
        compute(input, o)
            .unwrap()
            .ranking
            .into_iter()
            .map(|r| (r.node, r.score))
            .collect()
    }

    #[test]
    fn ranks_a_simple_link_graph() {
        // a → b → c → a with an extra a → c: c gains the most authority.
        let r = compute("a -> b\nb -> c\nc -> a\na -> c", &opts()).unwrap();
        assert_eq!(r.node_count, 3);
        assert_eq!(r.edge_count, 4);
        assert!(r.converged, "power iteration should converge");
        assert_eq!(r.ranking[0].node, "c");
        // Scores form a probability distribution.
        assert!((r.total_mass - 1.0).abs() < 1e-9, "mass {}", r.total_mass);
        let shares: f64 = r.ranking.iter().map(|x| x.share).sum();
        assert!((shares - 100.0).abs() < 1e-6);
    }

    #[test]
    fn matches_the_textbook_two_cycle() {
        // a ⇄ b is symmetric, so both nodes must land on exactly 0.5.
        let r = compute("a -> b\nb -> a", &opts()).unwrap();
        for row in &r.ranking {
            assert!(
                (row.score - 0.5).abs() < 1e-6,
                "{} = {}",
                row.node,
                row.score
            );
        }
    }

    #[test]
    fn damping_zero_gives_the_uniform_distribution() {
        let mut o = opts();
        o.damping = 0.0;
        let r = compute("a -> b\nb -> c\nc -> a\na -> c", &o).unwrap();
        for row in &r.ranking {
            assert!((row.score - 1.0 / 3.0).abs() < 1e-9, "{:?}", row);
        }
    }

    #[test]
    fn dangling_policies_differ() {
        let graph = "a -> b\nb -> c";
        let mut redistribute = opts();
        redistribute.dangling = "redistribute".into();
        let r = compute(graph, &redistribute).unwrap();
        assert!((r.total_mass - 1.0).abs() < 1e-6, "mass {}", r.total_mass);
        assert_eq!(r.dangling_nodes, vec!["c".to_string()]);

        let mut drop = opts();
        drop.dangling = "drop".into();
        let dropped = compute(graph, &drop).unwrap();
        assert!(
            dropped.total_mass < 0.9,
            "drop should leak mass, got {}",
            dropped.total_mass
        );

        let mut sink = opts();
        sink.dangling = "self-loop".into();
        let looped = compute(graph, &sink).unwrap();
        assert!((looped.total_mass - 1.0).abs() < 1e-6);
        // A self-looping sink accumulates the most authority.
        assert_eq!(looped.ranking[0].node, "c");
    }

    #[test]
    fn undirected_symmetrises_edges() {
        let mut o = opts();
        o.directed = false;
        // A star: the hub must outrank every leaf.
        let r = compute("hub - a\nhub - b\nhub - c", &o).unwrap();
        assert_eq!(r.ranking[0].node, "hub");
        assert_eq!(r.ranking[0].out_degree, 3);
        assert_eq!(r.ranking[0].in_degree, 3);
        assert!(r.dangling_nodes.is_empty());
    }

    #[test]
    fn weights_shift_the_ranking() {
        let mut o = opts();
        o.weighted = true;
        // hub splits its authority 9:1 in favour of `big`.
        let s = scores("seed -> hub : 1\nhub -> big : 9\nhub -> small : 1", &o);
        assert!(s["big"] > s["small"], "{s:?}");
        let mut unweighted = opts();
        unweighted.weighted = false;
        let u = scores(
            "seed -> hub : 1\nhub -> big : 9\nhub -> small : 1",
            &unweighted,
        );
        assert!((u["big"] - u["small"]).abs() < 1e-9, "{u:?}");
    }

    #[test]
    fn personalization_biases_the_jump() {
        let mut o = opts();
        o.personalization = "a:1".into();
        let r = compute("a -> b\nb -> a\nc -> a", &o).unwrap();
        assert!(r.personalized);
        let s: BTreeMap<String, f64> = r
            .ranking
            .iter()
            .map(|x| (x.node.clone(), x.score))
            .collect();
        // All teleport mass lands on `a`, so `c` (nothing points at it) is starved.
        assert!(s["a"] > s["b"], "{s:?}");
        assert!(s["c"] < 1e-9, "c should keep almost nothing: {s:?}");
    }

    #[test]
    fn parses_an_adjacency_matrix() {
        // Row = source. Node 3 is linked from both 1 and 2.
        let r = compute("0 1 1\n0 0 1\n1 0 0", &opts()).unwrap();
        assert_eq!(r.source_format, "adjacency matrix");
        assert_eq!(r.node_count, 3);
        assert_eq!(r.ranking[0].node, "3");
        let labelled = compute("x,y,z\nx,0,1,1\ny,0,0,1\nz,1,0,0", &{
            let mut o = opts();
            o.input_format = "matrix".into();
            o
        })
        .unwrap();
        assert_eq!(labelled.ranking[0].node, "z");
        // Same structure, so the scores match the unlabelled run.
        assert!((labelled.ranking[0].score - r.ranking[0].score).abs() < 1e-12);
    }

    #[test]
    fn top_truncates_but_keeps_the_totals() {
        let mut o = opts();
        o.top = 2;
        let r = compute("a -> b\nb -> c\nc -> a\na -> c", &o).unwrap();
        assert_eq!(r.ranking.len(), 2);
        assert_eq!(r.truncated, 1);
        assert!((r.total_mass - 1.0).abs() < 1e-9);
    }

    #[test]
    fn reports_non_convergence_instead_of_pretending() {
        let mut o = opts();
        o.max_iter = 2;
        o.tolerance = 1e-12;
        // Asymmetric, so the uniform start is not already the fixed point.
        let graph = "a -> b\nb -> c\nc -> a\na -> c";
        let r = compute(graph, &o).unwrap();
        assert!(!r.converged);
        assert_eq!(r.iterations, 2);
        let text = run(graph, &o).unwrap();
        assert!(text.contains("NOT CONVERGED"), "{text}");
    }

    #[test]
    fn isolated_nodes_still_rank() {
        let r = compute("a -> b\nlonely", &opts()).unwrap();
        assert_eq!(r.node_count, 3);
        let lonely = r.ranking.iter().find(|x| x.node == "lonely").unwrap();
        assert_eq!(lonely.in_degree, 0);
        assert!(lonely.dangling);
        assert!(lonely.score > 0.0);
    }

    #[test]
    fn parallel_edges_sum_their_weight() {
        let mut o = opts();
        o.weighted = true;
        let r = compute("a -> b : 2\na -> b : 3\na -> c : 1", &o).unwrap();
        assert_eq!(r.edge_count, 2);
        let b = r.ranking.iter().find(|x| x.node == "b").unwrap();
        assert!((b.in_weight - 5.0).abs() < 1e-12, "{:?}", b);
    }

    #[test]
    fn text_json_and_csv_agree() {
        let graph = "a -> b\nb -> c\nc -> a\na -> c";
        let text = run(graph, &opts()).unwrap();
        assert!(text.contains("PageRank — 3 nodes, 4 edges (directed, unweighted)"));
        assert!(text.contains("Total PageRank mass: 1.000000"));

        let mut j = opts();
        j.format = "json".into();
        let json = run(graph, &j).unwrap();
        assert!(json.contains("\"converged\": true"), "{json}");
        assert!(json.contains("\"node\": \"c\""), "{json}");

        let mut c = opts();
        c.format = "csv".into();
        let csv = run(graph, &c).unwrap();
        assert!(csv.starts_with("rank,node,pagerank,share_percent"));
        assert_eq!(csv.lines().count(), 4);
        assert!(csv.lines().nth(1).unwrap().starts_with("1,c,"));
    }

    #[test]
    fn decimals_control_the_score_width() {
        let mut o = opts();
        o.decimals = 2;
        let text = run("a -> b\nb -> a", &o).unwrap();
        assert!(text.contains("0.50"), "{text}");
        assert!(!text.contains("0.500"), "{text}");
    }

    // --- error paths -----------------------------------------------------

    #[test]
    fn rejects_empty_input() {
        let err = run("   \n# just a comment\n", &opts()).unwrap_err();
        assert!(err.contains("no graph"), "got: {err}");
    }

    #[test]
    fn rejects_a_negative_weight() {
        let mut o = opts();
        o.weighted = true;
        let err = compute("a -> b : -2", &o).unwrap_err();
        assert!(err.contains("negative weight"), "got: {err}");
        assert!(err.contains("line 1"), "got: {err}");
    }

    #[test]
    fn rejects_a_malformed_edge_line() {
        let err = compute("a -> b\na b c d", &opts()).unwrap_err();
        assert!(err.contains("line 2"), "got: {err}");
        assert!(err.contains("source target"), "got: {err}");
    }

    #[test]
    fn rejects_a_non_square_matrix() {
        let mut o = opts();
        o.input_format = "matrix".into();
        let err = compute("0 1 1\n1 0 1", &o).unwrap_err();
        assert!(err.contains("square"), "got: {err}");
    }

    #[test]
    fn rejects_out_of_range_damping() {
        let mut o = opts();
        o.damping = 1.0;
        let err = compute("a -> b", &o).unwrap_err();
        assert!(
            err.contains("damping must be between 0 and 0.999"),
            "got: {err}"
        );
    }

    #[test]
    fn rejects_unknown_enums() {
        let mut o = opts();
        o.dangling = "vanish".into();
        assert!(compute("a -> b", &o)
            .unwrap_err()
            .contains("invalid dangling"));
        let mut o = opts();
        o.format = "yaml".into();
        assert!(run("a -> b", &o).unwrap_err().contains("invalid format"));
        let mut o = opts();
        o.input_format = "dot".into();
        assert!(compute("a -> b", &o)
            .unwrap_err()
            .contains("invalid input_format"));
    }

    #[test]
    fn rejects_unknown_personalization_node() {
        let mut o = opts();
        o.personalization = "zz:1".into();
        let err = compute("a -> b", &o).unwrap_err();
        assert!(err.contains("not in the graph"), "got: {err}");
    }

    #[test]
    fn rejects_too_many_edges() {
        // Repeat one edge so the EDGE cap trips before the node cap.
        let at_cap = "a -> b\n".repeat(MAX_EDGES);
        assert!(
            compute(&at_cap, &opts()).is_ok(),
            "the cap itself must be accepted"
        );
        let over = format!("{at_cap}a -> b\n");
        let err = compute(&over, &opts()).unwrap_err();
        assert!(err.contains("too many edges"), "got: {err}");
    }

    #[test]
    fn rejects_too_many_nodes() {
        let mut graph = String::new();
        for i in 0..=MAX_NODES {
            graph.push_str(&format!("n{i}\n"));
        }
        let err = compute(&graph, &opts()).unwrap_err();
        assert!(err.contains("too many nodes"), "got: {err}");
    }
}
