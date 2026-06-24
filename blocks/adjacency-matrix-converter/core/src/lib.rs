//! adjacency-matrix-converter core — convert a graph between an edge list, an
//! adjacency matrix, and an incidence matrix. Pure compute, no wafer/wasm-bindgen
//! deps; shared by the chat skill block and the web page.
//!
//! Input formats (`from`):
//!   - `edges`     — one edge per line: `A B` (or `A B 3` when weighted). The two
//!                   endpoints are whitespace- or comma-separated labels; an
//!                   optional third token is the edge weight. A line with a single
//!                   token declares an isolated vertex. Blank lines and `#` comments
//!                   are ignored.
//!   - `adjacency` — a square numeric matrix, one row per line, entries separated by
//!                   whitespace or commas. An optional header row/column of labels is
//!                   auto-detected (when the first row/column is non-numeric).
//!
//! Output formats (`to`):
//!   - `adjacency` — labelled square adjacency matrix.
//!   - `incidence` — vertices × edges incidence matrix (see `to_incidence`).
//!   - `edges`     — normalized edge list (`A B` or `A B w`).
//!   - `list`      — adjacency list, one line per vertex: `A: B C`.
//!   - `degree`    — diagonal degree matrix (each vertex's weighted degree).
//!   - `laplacian` — graph Laplacian `L = D − A` (undirected graphs only).
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

/// Top-level conversion. `from`/`to` are the input/output formats (see module
/// docs); blank `from` → `edges`, blank `to` → `adjacency`.
pub fn convert(
    input: &str,
    from: &str,
    to: &str,
    directed: bool,
    weighted: bool,
) -> Result<String, String> {
    let graph = parse(input, from, directed, weighted)?;
    render(&graph, to)
}

/// Parse `input` (in format `from`) into a `Graph`.
pub fn parse(input: &str, from: &str, directed: bool, weighted: bool) -> Result<Graph, String> {
    match from {
        "" | "edges" => parse_edges(input, directed, weighted),
        "adjacency" => parse_adjacency(input, directed, weighted),
        other => Err(format!(
            "invalid 'from' {other:?}: expected \"edges\" or \"adjacency\""
        )),
    }
}

/// Render `graph` in format `to`.
pub fn render(graph: &Graph, to: &str) -> Result<String, String> {
    match to {
        "" | "adjacency" => Ok(to_adjacency(graph)),
        "incidence" => Ok(to_incidence(graph)),
        "edges" => Ok(to_edges(graph)),
        "list" => Ok(to_list(graph)),
        "degree" => Ok(to_degree(graph)),
        "laplacian" => to_laplacian(graph),
        other => Err(format!(
            "invalid 'to' {other:?}: expected \"adjacency\", \"incidence\", \"edges\", \"list\", \"degree\", or \"laplacian\""
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
        let toks = tokens(line);
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
                edges.push((a, b, weight));
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
    // Collect non-empty, non-comment rows of tokens.
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

    // Detect a header row: first row has a non-numeric token (the label header).
    let has_header_row = rows[0].iter().any(|t| !is_num(t));
    // Detect a leading label column: every data row's first token is non-numeric.
    let data_start = if has_header_row { 1 } else { 0 };
    let has_label_col = rows[data_start..]
        .iter()
        .all(|r| !r.is_empty() && !is_num(&r[0]));

    // Build the matrix of numeric cells and the labels.
    let mut labels: Vec<String> = Vec::new();
    let mut matrix: Vec<Vec<f64>> = Vec::new();

    if has_header_row {
        // Header row may or may not have a leading corner cell; take the labels
        // as everything in row 0 (skipping a leading non-numeric corner only if
        // a label column also exists and lengths line up).
        let header = &rows[0];
        labels = header.clone();
        // If there is also a label column, the header's first cell is a corner —
        // drop it when it makes the header length match the data width.
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
        } else if has_label_col {
            // Verify the row label matches the header if both present (best effort,
            // not enforced). Use the header labels as canonical.
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
    // If we have a header but no label column, labels are the header row.
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

    // Extract edges from the matrix.
    let mut edges: Vec<(usize, usize, f64)> = Vec::new();
    for i in 0..n {
        // For undirected graphs only read the upper triangle (incl. diagonal) to
        // avoid double-counting the symmetric entry.
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

/// Format a weight: integers print without a decimal point.
fn fmt_w(w: f64) -> String {
    if w.fract() == 0.0 && w.is_finite() {
        format!("{}", w as i64)
    } else {
        // Trim trailing zeros from a short fixed representation.
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

/// Render a vertices × edges incidence matrix. Columns are edges in input order,
/// labelled `e1, e2, …`. For an **undirected** graph an endpoint cell is the
/// weight (a self-loop is `2*weight` on its single vertex). For a **directed**
/// graph the tail is `-weight` and the head is `+weight` (a self-loop is `0`).
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

/// Render a normalized edge list. Isolated vertices (degree 0) are emitted as a
/// single-token line so a round-trip preserves them.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edges_to_undirected_adjacency() {
        let out = convert("A B\nB C\nA C", "edges", "adjacency", false, false).unwrap();
        // 3 vertices A,B,C; triangle.
        assert_eq!(
            out,
            " \tA\tB\tC\n\
             A\t0\t1\t1\n\
             B\t1\t0\t1\n\
             C\t1\t1\t0"
        );
    }

    #[test]
    fn edges_to_directed_adjacency() {
        let out = convert("A B\nB C", "edges", "adjacency", true, false).unwrap();
        assert_eq!(
            out,
            " \tA\tB\tC\n\
             A\t0\t1\t0\n\
             B\t0\t0\t1\n\
             C\t0\t0\t0"
        );
    }

    #[test]
    fn weighted_edges_to_adjacency() {
        let out = convert("A B 5\nB C 2.5", "edges", "adjacency", false, true).unwrap();
        assert_eq!(
            out,
            " \tA\tB\tC\n\
             A\t0\t5\t0\n\
             B\t5\t0\t2.5\n\
             C\t0\t2.5\t0"
        );
    }

    #[test]
    fn isolated_vertex_via_single_token() {
        let out = convert("A B\nD", "edges", "adjacency", false, false).unwrap();
        assert_eq!(
            out,
            " \tA\tB\tD\n\
             A\t0\t1\t0\n\
             B\t1\t0\t0\n\
             D\t0\t0\t0"
        );
    }

    #[test]
    fn comments_and_blanks_ignored() {
        let out = convert("# header\nA B\n\n  # mid\nB C", "edges", "edges", false, false).unwrap();
        assert_eq!(out, "A B\nB C");
    }

    #[test]
    fn adjacency_with_labels_to_edges() {
        let input = " \tA\tB\tC\nA\t0\t1\t1\nB\t1\t0\t0\nC\t1\t0\t0";
        let out = convert(input, "adjacency", "edges", false, false).unwrap();
        assert_eq!(out, "A B\nA C");
    }

    #[test]
    fn adjacency_no_labels_gets_default_labels() {
        let input = "0 1\n1 0";
        let out = convert(input, "adjacency", "edges", false, false).unwrap();
        assert_eq!(out, "V1 V2");
    }

    #[test]
    fn adjacency_weighted_round_trip() {
        let input = " A B\nA 0 7\nB 7 0";
        let out = convert(input, "adjacency", "edges", false, true).unwrap();
        assert_eq!(out, "A B 7");
    }

    #[test]
    fn directed_adjacency_reads_both_triangles() {
        let input = "0 1\n0 0";
        let edges = convert(input, "adjacency", "edges", true, false).unwrap();
        assert_eq!(edges, "V1 V2");
        // Symmetric entry is NOT auto-added for directed.
        let input2 = "0 0\n1 0";
        let edges2 = convert(input2, "adjacency", "edges", true, false).unwrap();
        assert_eq!(edges2, "V2 V1");
    }

    #[test]
    fn undirected_incidence_matrix() {
        // Triangle A-B, B-C: incidence has columns e1 (A-B), e2 (B-C).
        let out = convert("A B\nB C", "edges", "incidence", false, false).unwrap();
        assert_eq!(
            out,
            " \te1\te2\n\
             A\t1\t0\n\
             B\t1\t1\n\
             C\t0\t1"
        );
    }

    #[test]
    fn directed_incidence_signs_tail_negative() {
        let out = convert("A B", "edges", "incidence", true, false).unwrap();
        assert_eq!(
            out,
            " \te1\n\
             A\t-1\n\
             B\t1"
        );
    }

    #[test]
    fn rejects_non_square_matrix() {
        let err = convert("0 1 0\n1 0", "adjacency", "edges", false, false).unwrap_err();
        assert!(err.contains("not square"), "got: {err}");
    }

    #[test]
    fn rejects_bad_from() {
        let err = convert("A B", "bogus", "adjacency", false, false).unwrap_err();
        assert!(err.contains("invalid 'from'"), "got: {err}");
    }

    #[test]
    fn rejects_bad_to() {
        let err = convert("A B", "edges", "bogus", false, false).unwrap_err();
        assert!(err.contains("invalid 'to'"), "got: {err}");
    }

    #[test]
    fn rejects_bad_weight() {
        let err = convert("A B notanum", "edges", "adjacency", false, true).unwrap_err();
        assert!(err.contains("invalid weight"), "got: {err}");
    }

    #[test]
    fn rejects_empty_input() {
        let err = convert("\n# only a comment\n", "edges", "adjacency", false, false).unwrap_err();
        assert!(err.contains("no vertices"), "got: {err}");
    }

    #[test]
    fn to_adjacency_list() {
        let out = convert("A B\nA C\nB C", "edges", "list", false, false).unwrap();
        assert_eq!(out, "A: B C\nB: A C\nC: A B");
    }

    #[test]
    fn to_adjacency_list_weighted_and_isolated() {
        // D is isolated → "D:" with no neighbours.
        let out = convert("A B 3\nD", "edges", "list", false, true).unwrap();
        assert_eq!(out, "A: B(3)\nB: A(3)\nD:");
    }

    #[test]
    fn degree_matrix_diagonal() {
        // Triangle: every vertex has degree 2.
        let out = convert("A B\nB C\nA C", "edges", "degree", false, false).unwrap();
        assert_eq!(
            out,
            " \tA\tB\tC\n\
             A\t2\t0\t0\n\
             B\t0\t2\t0\n\
             C\t0\t0\t2"
        );
    }

    #[test]
    fn laplacian_is_degree_minus_adjacency() {
        // Path A-B-C: degrees 1,2,1; L = D - A.
        let out = convert("A B\nB C", "edges", "laplacian", false, false).unwrap();
        assert_eq!(
            out,
            " \tA\tB\tC\n\
             A\t1\t-1\t0\n\
             B\t-1\t2\t-1\n\
             C\t0\t-1\t1"
        );
    }

    #[test]
    fn laplacian_rejects_directed() {
        let err = convert("A B", "edges", "laplacian", true, false).unwrap_err();
        assert!(err.contains("undirected"), "got: {err}");
    }

    #[test]
    fn comma_separated_edges() {
        let out = convert("A,B\nB,C", "edges", "edges", false, false).unwrap();
        assert_eq!(out, "A B\nB C");
    }

    #[test]
    fn default_from_to_blank() {
        // blank from → edges, blank to → adjacency
        let out = convert("A B", "", "", false, false).unwrap();
        assert_eq!(out, " \tA\tB\nA\t0\t1\nB\t1\t0");
    }
}
