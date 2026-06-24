//! adjacency-matrix-converter core — convert a graph between an edge list, an
//! adjacency matrix, and an incidence matrix. Pure compute, no wafer/wasm-bindgen
//! deps; shared by the chat skill block and the web page.
//!
//! Input formats (`from`):
//!   - `auto`      — auto-detects the input format based on text patterns.
//!   - `edges`     — one edge per line: `A B` (or `A B 3` when weighted). Delimiters
//!                   like `->`, `--`, `-`, and `:` are automatically handled.
//!   - `adjacency` — a square numeric matrix, one row per line, entries separated by
//!                   whitespace or commas. An optional header row/column of labels is
//!                   auto-detected.
//!   - `list`      — adjacency list: `A: B C` or `A: B(3) C(1.5)` when weighted.
//!   - `incidence` — incidence matrix where rows represent vertices and columns edges.
//!
//! Output formats (`to`):
//!   - `adjacency` — labelled square adjacency matrix.
//!   - `incidence` — vertices × edges incidence matrix.
//!   - `edges`     — normalized edge list.
//!   - `list`      — adjacency list, one line per vertex: `A: B C`.
//!   - `degree`    — diagonal degree matrix (each vertex's weighted degree).
//!   - `laplacian` — graph Laplacian `L = D − A` (undirected graphs only).
//!   - `stats`     — analytical graph properties (nodes, edges, density, connectivity, cycles).
//!   - `power`     — walk-count matrix raised to power `k`.
//!
//! `directed` controls whether `A B` also implies `B A`. `weighted` controls
//! whether edge weights are read/emitted (unweighted collapses every edge to 1).

use std::collections::BTreeMap;
use std::fmt::Write as _;

/// Max vertices, to keep an `n×n` matrix bounded (and the output readable).
pub const MAX_VERTICES: usize = 200;

/// A parsed graph: an ordered vertex list plus a list of edges. Vertices keep
/// first-seen order so labels in the output match the order they appeared.
#[derive(Debug, Clone, PartialEq)]
pub struct Graph {
    pub vertices: Vec<String>,
    /// (from_index, to_index, weight). For an undirected graph each undirected
    /// edge is stored once (the lower-or-equal endpoint orientation is kept as
    /// parsed); the renderers mirror it as needed.
    pub edges: Vec<(usize, usize, f64)>,
    pub directed: bool,
    pub weighted: bool,
}

/// Top-level conversion.
pub fn convert(
    input: &str,
    from: &str,
    to: &str,
    directed: bool,
    weighted: bool,
    power: i64,
) -> Result<String, String> {
    let graph = parse(input, from, directed, weighted)?;
    render(&graph, to, power)
}

/// Parse `input` (in format `from`) into a `Graph`.
pub fn parse(input: &str, from: &str, directed: bool, weighted: bool) -> Result<Graph, String> {
    let resolved_from = match from {
        "" | "auto" => auto_detect_format(input),
        other => other,
    };
    match resolved_from {
        "edges" => parse_edges(input, directed, weighted),
        "adjacency" => parse_adjacency(input, directed, weighted),
        "list" => parse_list(input, directed, weighted),
        "incidence" => parse_incidence(input, directed, weighted),
        other => Err(format!(
            "invalid 'from' {other:?}: expected \"edges\", \"adjacency\", \"list\", or \"incidence\""
        )),
    }
}

/// Render `graph` in format `to`.
pub fn render(graph: &Graph, to: &str, power: i64) -> Result<String, String> {
    match to {
        "" | "adjacency" => Ok(to_adjacency(graph)),
        "incidence" => Ok(to_incidence(graph)),
        "edges" => Ok(to_edges(graph)),
        "list" => Ok(to_list(graph)),
        "degree" => Ok(to_degree(graph)),
        "laplacian" => to_laplacian(graph),
        "stats" => Ok(to_stats(graph)),
        "power" => to_power(graph, power),
        other => Err(format!(
            "invalid 'to' {other:?}: expected \"adjacency\", \"incidence\", \"edges\", \"list\", \"degree\", \"laplacian\", \"stats\", or \"power\""
        )),
    }
}

/// Split a line into tokens on whitespace and commas, dropping empties.
fn tokens(line: &str) -> Vec<&str> {
    line.split(|c: char| c.is_whitespace() || c == ',')
        .filter(|t| !t.is_empty())
        .collect()
}

/// Strip a trailing inline `#` comment and surrounding whitespace.
fn strip_comment(line: &str) -> &str {
    match line.find('#') {
        Some(i) => &line[..i],
        None => line,
    }
}

/// Clean edge list line by removing typical delimiters.
fn clean_edge_line(line: &str) -> String {
    let mut s = line.to_string();
    s = s.replace("->", " ");
    s = s.replace("--", " ");
    s = s.replace(" - ", " ");
    s = s.replace(":", " ");
    s
}

fn parse_edges(input: &str, directed: bool, weighted: bool) -> Result<Graph, String> {
    let mut index: BTreeMap<String, usize> = BTreeMap::new();
    let mut vertices: Vec<String> = Vec::new();
    let mut edges: Vec<(usize, usize, f64)> = Vec::new();

    let vid = |label: &str,
                   vertices: &mut Vec<String>,
                   index: &mut BTreeMap<String, usize>|
     -> Result<usize, String> {
        if let Some(&i) = index.get(label) {
            return Ok(i);
        }
        if vertices.len() >= MAX_VERTICES {
            return Err(format!("too many vertices (max {MAX_VERTICES})"));
        }
        let i = vertices.len();
        vertices.push(label.to_string());
        index.insert(label.to_string(), i);
        Ok(i)
    };

    for (lineno, raw) in input.lines().enumerate() {
        let line = strip_comment(raw).trim();
        if line.is_empty() {
            continue;
        }
        let cleaned = clean_edge_line(line);
        let toks = tokens(&cleaned);
        match toks.len() {
            // Single token: declare an isolated vertex.
            1 => {
                vid(toks[0], &mut vertices, &mut index)?;
            }
            2 | 3 => {
                let a = vid(toks[0], &mut vertices, &mut index)?;
                let b = vid(toks[1], &mut vertices, &mut index)?;
                let weight = if weighted && toks.len() == 3 {
                    toks[2].parse::<f64>().map_err(|_| {
                        format!("line {}: invalid weight {:?}", lineno + 1, toks[2])
                    })?
                } else {
                    1.0
                };
                let (u, v) = if !directed && a > b { (b, a) } else { (a, b) };
                if !edges.iter().any(|&(x, y, _)| x == u && y == v) {
                    edges.push((u, v, weight));
                }
            }
            n => {
                return Err(format!(
                    "line {}: expected 1-3 tokens (vertex, or 'from to [weight]'), got {n}",
                    lineno + 1
                ))
            }
        }
    }

    if vertices.is_empty() {
        return Err("no vertices found in input".into());
    }

    Ok(Graph {
        vertices,
        edges,
        directed,
        weighted,
    })
}

fn parse_adjacency(input: &str, directed: bool, weighted: bool) -> Result<Graph, String> {
    let rows: Vec<Vec<String>> = input
        .lines()
        .map(|l| strip_comment(l).trim())
        .filter(|l| !l.is_empty())
        .map(|l| tokens(l).into_iter().map(|t| t.to_string()).collect())
        .collect();
    if rows.is_empty() {
        return Err("no rows found in input".into());
    }

    let is_num = |s: &str| s.parse::<f64>().is_ok();

    let has_header_row = rows[0].iter().any(|t| !is_num(t));
    let data_start = if has_header_row { 1 } else { 0 };
    let has_label_col = rows[data_start..]
        .iter()
        .all(|r| !r.is_empty() && !is_num(&r[0]));

    let mut labels: Vec<String> = Vec::new();
    let mut matrix: Vec<Vec<f64>> = Vec::new();

    if has_header_row {
        let header = &rows[0];
        labels = header.clone();
        if has_label_col && labels.len() == rows[data_start..][0].len() {
            labels.remove(0);
        }
    }

    for (ri, row) in rows[data_start..].iter().enumerate() {
        let (label, nums) = if has_label_col {
            (row[0].clone(), &row[1..])
        } else {
            (String::new(), &row[..])
        };
        if !has_header_row {
            labels.push(if label.is_empty() {
                format!("V{}", ri + 1)
            } else {
                label
            });
        }
        let parsed: Result<Vec<f64>, String> = nums
            .iter()
            .map(|t| t.parse::<f64>().map_err(|_| format!("non-numeric matrix cell {t:?}")))
            .collect();
        matrix.push(parsed?);
    }

    let n = matrix.len();
    if n > MAX_VERTICES {
        return Err(format!("too many vertices (max {MAX_VERTICES})"));
    }
    if labels.is_empty() {
        labels = (1..=n).map(|i| format!("V{i}")).collect();
    }
    if labels.len() != n {
        return Err(format!(
            "matrix is {n}×{} but has {} labels — rows must match labels",
            matrix.first().map(|r| r.len()).unwrap_or(0),
            labels.len()
        ));
    }
    for (i, row) in matrix.iter().enumerate() {
        if row.len() != n {
            return Err(format!(
                "matrix is not square: row {} has {} columns, expected {n}",
                i + 1,
                row.len()
            ));
        }
    }

    let mut edges: Vec<(usize, usize, f64)> = Vec::new();
    for i in 0..n {
        let j_start = if directed { 0 } else { i };
        for j in j_start..n {
            let w = matrix[i][j];
            if w != 0.0 {
                let weight = if weighted { w } else { 1.0 };
                edges.push((i, j, weight));
            }
        }
    }

    Ok(Graph {
        vertices: labels,
        edges,
        directed,
        weighted,
    })
}

fn parse_list(input: &str, directed: bool, weighted: bool) -> Result<Graph, String> {
    let mut index: BTreeMap<String, usize> = BTreeMap::new();
    let mut vertices: Vec<String> = Vec::new();
    let mut edges: Vec<(usize, usize, f64)> = Vec::new();

    let vid = |label: &str,
                   vertices: &mut Vec<String>,
                   index: &mut BTreeMap<String, usize>|
     -> Result<usize, String> {
        if let Some(&i) = index.get(label) {
            return Ok(i);
        }
        if vertices.len() >= MAX_VERTICES {
            return Err(format!("too many vertices (max {MAX_VERTICES})"));
        }
        let i = vertices.len();
        vertices.push(label.to_string());
        index.insert(label.to_string(), i);
        Ok(i)
    };

    for (lineno, raw) in input.lines().enumerate() {
        let line = strip_comment(raw).trim();
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.splitn(2, ':').collect();
        let source_label = parts[0].trim();
        if source_label.is_empty() {
            return Err(format!("line {}: empty source node", lineno + 1));
        }
        let source_id = vid(source_label, &mut vertices, &mut index)?;

        if parts.len() > 1 {
            let target_tokens = tokens(parts[1]);
            for tok in target_tokens {
                let mut target_label = tok;
                let mut weight = 1.0;
                if let Some(start) = tok.find('(') {
                    if let Some(end) = tok.find(')') {
                        if end > start {
                            target_label = &tok[..start];
                            if weighted {
                                weight = tok[start+1..end].parse::<f64>().map_err(|_| {
                                    format!("line {}: invalid weight in token {:?}", lineno + 1, tok)
                                })?;
                            }
                        }
                    }
                } else if let Some(idx) = tok.find(':') {
                    target_label = &tok[..idx];
                    if weighted {
                        weight = tok[idx+1..].parse::<f64>().map_err(|_| {
                            format!("line {}: invalid weight in token {:?}", lineno + 1, tok)
                        })?;
                    }
                }
                let target_id = vid(target_label, &mut vertices, &mut index)?;
                let (u, v) = if !directed && source_id > target_id { (target_id, source_id) } else { (source_id, target_id) };
                if !edges.iter().any(|&(x, y, _)| x == u && y == v) {
                    edges.push((u, v, weight));
                }
            }
        }
    }

    if vertices.is_empty() {
        return Err("no vertices found in input".into());
    }

    Ok(Graph {
        vertices,
        edges,
        directed,
        weighted,
    })
}

fn parse_incidence(input: &str, directed: bool, weighted: bool) -> Result<Graph, String> {
    let rows: Vec<Vec<String>> = input
        .lines()
        .map(|l| strip_comment(l).trim())
        .filter(|l| !l.is_empty())
        .map(|l| tokens(l).into_iter().map(|t| t.to_string()).collect())
        .collect();
    if rows.is_empty() {
        return Err("no rows found in input".into());
    }

    let is_num = |s: &str| s.parse::<f64>().is_ok();

    let has_header_row = rows[0].iter().any(|t| !is_num(t));
    let data_start = if has_header_row { 1 } else { 0 };
    let has_label_col = rows[data_start..]
        .iter()
        .all(|r| !r.is_empty() && !is_num(&r[0]));

    let num_rows = rows.len() - data_start;
    if num_rows == 0 {
        return Err("no data rows found in incidence matrix".into());
    }

    let first_row = &rows[data_start];
    let num_cols = if has_label_col {
        if first_row.is_empty() { 0 } else { first_row.len() - 1 }
    } else {
        first_row.len()
    };

    if num_cols == 0 {
        return Err("incidence matrix has 0 columns".into());
    }

    let mut labels: Vec<String> = Vec::new();
    let mut grid: Vec<Vec<f64>> = Vec::new();

    for (ri, row) in rows[data_start..].iter().enumerate() {
        let (label, nums) = if has_label_col {
            (row[0].clone(), &row[1..])
        } else {
            (String::new(), &row[..])
        };
        labels.push(if label.is_empty() {
            format!("V{}", ri + 1)
        } else {
            label
        });
        if nums.len() != num_cols {
            return Err(format!(
                "row {} of incidence matrix has {} entries, expected {}",
                ri + 1 + data_start,
                nums.len(),
                num_cols
            ));
        }
        let parsed: Result<Vec<f64>, String> = nums
            .iter()
            .map(|t| t.parse::<f64>().map_err(|_| format!("non-numeric incidence cell {t:?}")))
            .collect();
        grid.push(parsed?);
    }

    let mut edges: Vec<(usize, usize, f64)> = Vec::new();

    for col in 0..num_cols {
        let mut non_zeros = Vec::new();
        for row in 0..num_rows {
            let val = grid[row][col];
            if val != 0.0 {
                non_zeros.push((row, val));
            }
        }

        if directed {
            let tail_opt = non_zeros.iter().find(|&&(_, val)| val < 0.0);
            let head_opt = non_zeros.iter().find(|&&(_, val)| val > 0.0);

            match (tail_opt, head_opt) {
                (Some(&(tail_idx, _)), Some(&(head_idx, head_val))) => {
                    let w = if weighted { head_val } else { 1.0 };
                    edges.push((tail_idx, head_idx, w));
                }
                _ => {
                    if non_zeros.len() == 1 {
                        let &(idx, val) = &non_zeros[0];
                        let w = if weighted { val.abs() } else { 1.0 };
                        edges.push((idx, idx, w));
                    } else {
                        return Err(format!(
                            "directed incidence matrix column {} must have one negative (tail) and one positive (head) value",
                            col + 1
                        ));
                    }
                }
            }
        } else {
            match non_zeros.len() {
                1 => {
                    let &(idx, val) = &non_zeros[0];
                    let w = if weighted { val / 2.0 } else { 1.0 };
                    edges.push((idx, idx, w));
                }
                2 => {
                    let &(idx1, val1) = &non_zeros[0];
                    let &(idx2, _) = &non_zeros[1];
                    let w = if weighted { val1 } else { 1.0 };
                    let (u, v) = if idx1 > idx2 { (idx2, idx1) } else { (idx1, idx2) };
                    if !edges.iter().any(|&(x, y, _)| x == u && y == v) {
                        edges.push((u, v, w));
                    }
                }
                0 => {
                    return Err(format!("incidence matrix column {} is all zeros", col + 1));
                }
                _ => {
                    return Err(format!(
                        "undirected incidence matrix column {} must have at most 2 non-zero entries",
                        col + 1
                    ));
                }
            }
        }
    }

    Ok(Graph {
        vertices: labels,
        edges,
        directed,
        weighted,
    })
}

fn auto_detect_format(input: &str) -> &'static str {
    let lines: Vec<&str> = input
        .lines()
        .map(|l| strip_comment(l).trim())
        .filter(|l| !l.is_empty())
        .collect();
    if lines.is_empty() {
        return "edges";
    }

    let list_hits = lines.iter().filter(|l| {
        if let Some(idx) = l.find(':') {
            let left = l[..idx].trim();
            !left.is_empty() && left.split_whitespace().count() == 1
        } else {
            false
        }
    }).count();

    if list_hits > 0 && list_hits >= lines.len() / 2 {
        return "list";
    }

    let cleaned_lines: Vec<String> = lines.iter().map(|l| clean_edge_line(l)).collect();
    let token_counts: Vec<String> = cleaned_lines.iter().map(|l| l.clone()).collect();
    let token_lens: Vec<usize> = token_counts.iter().map(|l| tokens(l).len()).collect();
    let min_toks = token_lens.iter().min().copied().unwrap_or(0);
    let max_toks = token_lens.iter().max().copied().unwrap_or(0);

    if min_toks == max_toks && min_toks > 1 {
        let is_num = |s: &str| s.parse::<f64>().is_ok();
        let first_row_toks = tokens(&cleaned_lines[0]);
        let has_header_row = first_row_toks.iter().any(|t| !is_num(t));
        let data_start = if has_header_row { 1 } else { 0 };
        
        let has_label_col = cleaned_lines[data_start..]
            .iter()
            .all(|l| tokens(l).first().map(|t| !is_num(t)).unwrap_or(false));

        let num_rows = cleaned_lines.len() - data_start;
        let num_cols = if has_label_col { min_toks - 1 } else { min_toks };

        let mut looks_like_matrix = true;
        for l in &cleaned_lines[data_start..] {
            let row_toks = tokens(l);
            let start_idx = if has_label_col { 1 } else { 0 };
            if row_toks.len() <= start_idx {
                looks_like_matrix = false;
                break;
            }
            for tok in &row_toks[start_idx..] {
                if !is_num(tok) {
                    looks_like_matrix = false;
                    break;
                }
            }
            if !looks_like_matrix {
                break;
            }
        }

        if looks_like_matrix {
            if num_rows == num_cols {
                return "adjacency";
            } else {
                return "incidence";
            }
        }
    }

    "edges"
}

/// Format a weight: integers print without a decimal point.
fn fmt_w(w: f64) -> String {
    if w.fract() == 0.0 && w.is_finite() {
        format!("{}", w as i64)
    } else {
        let s = format!("{w}");
        s
    }
}

/// Build the dense `n×n` adjacency matrix. Parallel edges sum; for an undirected
/// graph the symmetric cell is mirrored.
fn dense_adjacency(graph: &Graph) -> Vec<Vec<f64>> {
    let n = graph.vertices.len();
    let mut m = vec![vec![0.0f64; n]; n];
    for &(a, b, w) in &graph.edges {
        let v = if graph.weighted { w } else { 1.0 };
        m[a][b] += v;
        if !graph.directed && a != b {
            m[b][a] += v;
        }
    }
    m
}

/// Render an `n×n` matrix with vertex labels as the header row and first column.
fn render_labelled_square(graph: &Graph, m: &[Vec<f64>]) -> String {
    let n = graph.vertices.len();
    let mut out = String::new();
    out.push(' ');
    for label in &graph.vertices {
        let _ = write!(out, "\t{label}");
    }
    out.push('\n');
    for i in 0..n {
        let _ = write!(out, "{}", graph.vertices[i]);
        for j in 0..n {
            let _ = write!(out, "\t{}", fmt_w(m[i][j]));
        }
        out.push('\n');
    }
    out.trim_end().to_string()
}

/// Render a labelled square adjacency matrix. The first row/column are labels.
pub fn to_adjacency(graph: &Graph) -> String {
    render_labelled_square(graph, &dense_adjacency(graph))
}

/// Render an adjacency list, one line per vertex: `A: B C` (neighbours in label
/// order; weighted neighbours show as `B(3)`). A vertex with no out-edges prints
/// just `A:`.
pub fn to_list(graph: &Graph) -> String {
    let n = graph.vertices.len();
    let m = dense_adjacency(graph);
    let mut out = String::new();
    for i in 0..n {
        let _ = write!(out, "{}:", graph.vertices[i]);
        for j in 0..n {
            if m[i][j] != 0.0 {
                if graph.weighted {
                    let _ = write!(out, " {}({})", graph.vertices[j], fmt_w(m[i][j]));
                } else {
                    let _ = write!(out, " {}", graph.vertices[j]);
                }
            }
        }
        out.push('\n');
    }
    out.trim_end().to_string()
}

/// Each vertex's (weighted) degree. For a directed graph this is the out-degree
/// row-sum of the adjacency matrix.
fn degrees(graph: &Graph) -> Vec<f64> {
    let m = dense_adjacency(graph);
    m.iter().map(|row| row.iter().sum()).collect()
}

/// Render the diagonal degree matrix `D`.
pub fn to_degree(graph: &Graph) -> String {
    let n = graph.vertices.len();
    let deg = degrees(graph);
    let mut m = vec![vec![0.0f64; n]; n];
    for i in 0..n {
        m[i][i] = deg[i];
    }
    render_labelled_square(graph, &m)
}

/// Render the graph Laplacian `L = D − A` (undirected only). Each diagonal is the
/// vertex degree; off-diagonals are `−A[i][j]`.
pub fn to_laplacian(graph: &Graph) -> Result<String, String> {
    if graph.directed {
        return Err("laplacian is only defined for an undirected graph (set directed=false)".into());
    }
    let n = graph.vertices.len();
    let a = dense_adjacency(graph);
    let deg = degrees(graph);
    let mut l = vec![vec![0.0f64; n]; n];
    for i in 0..n {
        for j in 0..n {
            l[i][j] = if i == j { deg[i] - a[i][j] } else { -a[i][j] };
        }
    }
    Ok(render_labelled_square(graph, &l))
}

/// Render a vertices × edges incidence matrix.
pub fn to_incidence(graph: &Graph) -> String {
    let n = graph.vertices.len();
    let e = graph.edges.len();
    let mut m = vec![vec![0.0f64; e]; n];
    for (col, &(a, b, w)) in graph.edges.iter().enumerate() {
        let v = if graph.weighted { w } else { 1.0 };
        if graph.directed {
            if a == b {
                // self-loop in a directed graph contributes net 0
            } else {
                m[a][col] -= v;
                m[b][col] += v;
            }
        } else if a == b {
            m[a][col] += 2.0 * v;
        } else {
            m[a][col] += v;
            m[b][col] += v;
        }
    }

    let mut out = String::new();
    out.push(' ');
    for k in 1..=e {
        let _ = write!(out, "\te{k}");
    }
    out.push('\n');
    for i in 0..n {
        let _ = write!(out, "{}", graph.vertices[i]);
        for col in 0..e {
            let _ = write!(out, "\t{}", fmt_w(m[i][col]));
        }
        out.push('\n');
    }
    out.trim_end().to_string()
}

/// Render a normalized edge list.
pub fn to_edges(graph: &Graph) -> String {
    let mut out = String::new();
    let mut seen = vec![false; graph.vertices.len()];
    for &(a, b, w) in &graph.edges {
        seen[a] = true;
        seen[b] = true;
        if graph.weighted {
            let _ = writeln!(
                out,
                "{} {} {}",
                graph.vertices[a],
                graph.vertices[b],
                fmt_w(w)
            );
        } else {
            let _ = writeln!(out, "{} {}", graph.vertices[a], graph.vertices[b]);
        }
    }
    for (i, used) in seen.iter().enumerate() {
        if !used {
            let _ = writeln!(out, "{}", graph.vertices[i]);
        }
    }
    out.trim_end().to_string()
}

// Matrix multiplication
fn multiply(a: &[Vec<f64>], b: &[Vec<f64>]) -> Vec<Vec<f64>> {
    let n = a.len();
    let mut c = vec![vec![0.0; n]; n];
    for i in 0..n {
        for j in 0..n {
            let mut sum = 0.0;
            for k in 0..n {
                sum += a[i][k] * b[k][j];
            }
            c[i][j] = sum;
        }
    }
    c
}

pub fn to_power(graph: &Graph, k: i64) -> Result<String, String> {
    if k < 1 || k > 10 {
        return Err("power must be between 1 and 10".into());
    }
    let a = dense_adjacency(graph);
    let mut res = a.clone();
    for _ in 1..k {
        res = multiply(&res, &a);
    }
    Ok(render_labelled_square(graph, &res))
}

// Helper connectivity / cycle detection functions
fn get_undirected_adjacency(graph: &Graph) -> Vec<Vec<usize>> {
    let n = graph.vertices.len();
    let mut adj = vec![vec![]; n];
    for &(a, b, _) in &graph.edges {
        adj[a].push(b);
        if a != b {
            adj[b].push(a);
        }
    }
    adj
}

fn get_directed_adjacency(graph: &Graph) -> Vec<Vec<usize>> {
    let n = graph.vertices.len();
    let mut adj = vec![vec![]; n];
    for &(a, b, _) in &graph.edges {
        adj[a].push(b);
    }
    adj
}

fn is_connected_undirected(graph: &Graph) -> bool {
    let n = graph.vertices.len();
    if n <= 1 { return true; }
    let adj = get_undirected_adjacency(graph);
    let mut visited = vec![false; n];
    let mut q = std::collections::VecDeque::new();
    q.push_back(0);
    visited[0] = true;
    let mut count = 1;
    while let Some(u) = q.pop_front() {
        for &v in &adj[u] {
            if !visited[v] {
                visited[v] = true;
                q.push_back(v);
                count += 1;
            }
        }
    }
    count == n
}

fn is_weakly_connected(graph: &Graph) -> bool {
    is_connected_undirected(graph)
}

fn is_strongly_connected(graph: &Graph) -> bool {
    let n = graph.vertices.len();
    if n <= 1 { return true; }
    let adj = get_directed_adjacency(graph);
    for start in 0..n {
        let mut visited = vec![false; n];
        let mut q = std::collections::VecDeque::new();
        q.push_back(start);
        visited[start] = true;
        let mut count = 1;
        while let Some(u) = q.pop_front() {
            for &v in &adj[u] {
                if !visited[v] {
                    visited[v] = true;
                    q.push_back(v);
                    count += 1;
                }
            }
        }
        if count != n { return false; }
    }
    true
}

fn has_cycle_undirected(graph: &Graph) -> bool {
    let n = graph.vertices.len();
    let adj = get_undirected_adjacency(graph);
    let mut visited = vec![false; n];

    fn dfs(u: usize, parent: Option<usize>, adj: &[Vec<usize>], visited: &mut [bool]) -> bool {
        visited[u] = true;
        for &v in &adj[u] {
            if !visited[v] {
                if dfs(v, Some(u), adj, visited) {
                    return true;
                }
            } else if Some(v) != parent {
                return true;
            }
        }
        false
    }

    for i in 0..n {
        if !visited[i] {
            if dfs(i, None, &adj, &mut visited) {
                return true;
            }
        }
    }
    false
}

fn has_cycle_directed(graph: &Graph) -> bool {
    let n = graph.vertices.len();
    let adj = get_directed_adjacency(graph);
    let mut state = vec![0; n];

    fn dfs(u: usize, adj: &[Vec<usize>], state: &mut [i32]) -> bool {
        state[u] = 1;
        for &v in &adj[u] {
            if state[v] == 1 {
                return true;
            } else if state[v] == 0 {
                if dfs(v, adj, state) {
                    return true;
                }
            }
        }
        state[u] = 2;
        false
    }

    for i in 0..n {
        if state[i] == 0 {
            if dfs(i, &adj, &mut state) {
                return true;
            }
        }
    }
    false
}

pub fn to_stats(graph: &Graph) -> String {
    let v_count = graph.vertices.len();
    let e_count = graph.edges.len();
    let directed = graph.directed;
    let weighted = graph.weighted;

    let density = if v_count <= 1 {
        0.0
    } else {
        let max_edges = (v_count * (v_count - 1)) as f64;
        if directed {
            e_count as f64 / max_edges
        } else {
            (2 * e_count) as f64 / max_edges
        }
    };

    let mut out = String::new();
    let _ = writeln!(out, "Vertices: {v_count}");
    let _ = writeln!(out, "Edges: {e_count}");
    let _ = writeln!(out, "Directed: {}", if directed { "Yes" } else { "No" });
    let _ = writeln!(out, "Weighted: {}", if weighted { "Yes" } else { "No" });
    let _ = writeln!(out, "Density: {:.4}", density);

    if directed {
        let weak = is_weakly_connected(graph);
        let strong = is_strongly_connected(graph);
        let _ = writeln!(out, "Weakly Connected: {}", if weak { "Yes" } else { "No" });
        let _ = writeln!(out, "Strongly Connected: {}", if strong { "Yes" } else { "No" });
    } else {
        let conn = is_connected_undirected(graph);
        let _ = writeln!(out, "Connected: {}", if conn { "Yes" } else { "No" });
    }

    let has_cycle = if directed {
        has_cycle_directed(graph)
    } else {
        has_cycle_undirected(graph)
    };
    let _ = writeln!(out, "Has Cycles: {}", if has_cycle { "Yes" } else { "No" });

    if directed {
        let mut in_degs = vec![0.0; v_count];
        let mut out_degs = vec![0.0; v_count];
        for &(a, b, w) in &graph.edges {
            let val = if weighted { w } else { 1.0 };
            out_degs[a] += val;
            in_degs[b] += val;
        }

        let format_deg_list = |degs: &[f64], labels: &[String]| -> String {
            labels.iter().zip(degs.iter())
                .map(|(lbl, d)| format!("{lbl}:{}", fmt_w(*d)))
                .collect::<Vec<String>>()
                .join(", ")
        };

        let min_in = in_degs.iter().copied().fold(f64::INFINITY, f64::min);
        let max_in = in_degs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let avg_in = in_degs.iter().sum::<f64>() / v_count as f64;

        let min_out = out_degs.iter().copied().fold(f64::INFINITY, f64::min);
        let max_out = out_degs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let avg_out = out_degs.iter().sum::<f64>() / v_count as f64;

        let _ = writeln!(out, "In-Degree Sequence: {}", format_deg_list(&in_degs, &graph.vertices));
        let _ = writeln!(out, "In-Degree Min/Max/Avg: {} / {} / {:.2}", fmt_w(min_in), fmt_w(max_in), avg_in);
        let _ = writeln!(out, "Out-Degree Sequence: {}", format_deg_list(&out_degs, &graph.vertices));
        let _ = writeln!(out, "Out-Degree Min/Max/Avg: {} / {} / {:.2}", fmt_w(min_out), fmt_w(max_out), avg_out);
    } else {
        let mut degs = vec![0.0; v_count];
        for &(a, b, w) in &graph.edges {
            let val = if weighted { w } else { 1.0 };
            degs[a] += val;
            if a != b {
                degs[b] += val;
            } else {
                degs[a] += val;
            }
        }

        let format_deg_list = |degs: &[f64], labels: &[String]| -> String {
            labels.iter().zip(degs.iter())
                .map(|(lbl, d)| format!("{lbl}:{}", fmt_w(*d)))
                .collect::<Vec<String>>()
                .join(", ")
        };

        let min_deg = degs.iter().copied().fold(f64::INFINITY, f64::min);
        let max_deg = degs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let avg_deg = degs.iter().sum::<f64>() / v_count as f64;

        let _ = writeln!(out, "Degree Sequence: {}", format_deg_list(&degs, &graph.vertices));
        let _ = writeln!(out, "Degree Min/Max/Avg: {} / {} / {:.2}", fmt_w(min_deg), fmt_w(max_deg), avg_deg);
    }

    out.trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edges_to_undirected_adjacency() {
        let out = convert("A B\nB C\nA C", "edges", "adjacency", false, false, 2).unwrap();
        assert_eq!(
            out,
            " \tA\tB\tC\n\
             A\t0\t1\t1\n\
             B\t1\t0\t1\n\
             C\t1\t1\t0"
        );
    }

    #[test]
    fn clean_edges_arrows_dashes() {
        let out = convert("A -> B\nB -- C\nA - C", "edges", "adjacency", false, false, 2).unwrap();
        assert_eq!(
            out,
            " \tA\tB\tC\n\
             A\t0\t1\t1\n\
             B\t1\t0\t1\n\
             C\t1\t1\t0"
        );
    }

    #[test]
    fn auto_detect_edges() {
        let out = convert("A -> B\nB -> C", "auto", "edges", true, false, 2).unwrap();
        assert_eq!(out, "A B\nB C");
    }

    #[test]
    fn parse_adjacency_list_format() {
        let out = convert("A: B C\nB: A C\nC: A B", "list", "edges", false, false, 2).unwrap();
        assert_eq!(out, "A B\nA C\nB C");
    }

    #[test]
    fn parse_incidence_matrix_format() {
        let input = " \te1\te2\nA\t1\t0\nB\t1\t1\nC\t0\t1";
        let out = convert(input, "incidence", "edges", false, false, 2).unwrap();
        assert_eq!(out, "A B\nB C");
    }

    #[test]
    fn compute_stats_mode() {
        let out = convert("A B\nB C\nA C", "edges", "stats", false, false, 2).unwrap();
        assert!(out.contains("Vertices: 3"));
        assert!(out.contains("Edges: 3"));
        assert!(out.contains("Connected: Yes"));
        assert!(out.contains("Has Cycles: Yes"));
    }

    #[test]
    fn compute_matrix_powers() {
        let out = convert("A B\nB C", "edges", "power", false, false, 2).unwrap();
        assert_eq!(
            out,
            " \tA\tB\tC\n\
             A\t1\t0\t1\n\
             B\t0\t2\t0\n\
             C\t1\t0\t1"
        );
    }
}
