//! graph-algorithms core — pure compute, shared by the chat skill block and the
//! web page. No wafer/wasm-bindgen deps.
//!
//! Parses a user-supplied edge list and runs one of several classic graph
//! algorithms: breadth-first search (BFS), depth-first search (DFS),
//! shortest path (Dijkstra / unit-weight BFS), topological sort, and cycle
//! detection. Output is a small human-readable report.

use std::collections::{BTreeMap, BTreeSet, BinaryHeap, VecDeque};

/// Parsed adjacency list. Nodes are arbitrary string labels. Each edge stores a
/// non-negative weight (defaults to 1 when the input is unweighted).
struct Graph {
    /// All node labels, in first-seen order so output is deterministic and
    /// reflects the input.
    nodes: Vec<String>,
    /// node -> list of (neighbor, weight). Neighbor lists are sorted by label
    /// after parsing for deterministic traversal.
    adj: BTreeMap<String, Vec<(String, f64)>>,
    directed: bool,
}

impl Graph {
    fn neighbors(&self, n: &str) -> &[(String, f64)] {
        self.adj.get(n).map(|v| v.as_slice()).unwrap_or(&[])
    }
}

/// Parse an edge list. Accepted line formats (one edge or node per line):
///   `a -> b`        directed-style arrow
///   `a - b` / `a b` / `a,b`   plain edge
///   `a -> b : 3` or `a b 3`   weighted edge (trailing number is the weight)
///   `a`             isolated node (no edge)
/// Blank lines and lines starting with `#` are ignored. Whether the graph is
/// treated as directed is controlled by `directed`, not the arrow glyph.
fn parse_graph(input: &str, directed: bool, weighted: bool) -> Result<Graph, String> {
    let mut nodes: Vec<String> = Vec::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut adj: BTreeMap<String, Vec<(String, f64)>> = BTreeMap::new();

    fn add_node(label: &str, nodes: &mut Vec<String>, seen: &mut BTreeSet<String>) {
        if seen.insert(label.to_string()) {
            nodes.push(label.to_string());
        }
    }

    let mut had_any = false;
    for (lineno, raw) in input.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        had_any = true;
        // Normalize separators into spaces, then split on whitespace. Only a
        // *spaced* hyphen (` - `) and the arrow/double-dash glyphs are treated
        // as edge separators, so a bare `-` inside a label or a negative weight
        // (`-2`) survives. `,` and `:` are always separators.
        let norm = line
            .replace("->", " ")
            .replace(" -- ", " ")
            .replace(" - ", " ")
            .replace(',', " ")
            .replace(':', " ");
        let toks: Vec<&str> = norm.split_whitespace().collect();
        if toks.is_empty() {
            continue;
        }
        if toks.len() == 1 {
            // Isolated node.
            add_node(toks[0], &mut nodes, &mut seen);
            adj.entry(toks[0].to_string()).or_default();
            continue;
        }

        // Optional trailing weight: the last token parses as a number AND there
        // are at least 3 tokens (src dst weight).
        let (a, b, w) = if weighted && toks.len() >= 3 {
            if let Ok(parsed) = toks[toks.len() - 1].parse::<f64>() {
                if parsed < 0.0 {
                    return Err(format!(
                        "line {}: negative weight {} is not allowed (Dijkstra requires non-negative weights)",
                        lineno + 1,
                        parsed
                    ));
                }
                (toks[0], toks[1], parsed)
            } else {
                (toks[0], toks[1], 1.0)
            }
        } else {
            (toks[0], toks[1], 1.0)
        };

        add_node(a, &mut nodes, &mut seen);
        add_node(b, &mut nodes, &mut seen);
        adj.entry(a.to_string()).or_default().push((b.to_string(), w));
        adj.entry(b.to_string()).or_default(); // ensure b is a key
        if !directed {
            adj.entry(b.to_string()).or_default().push((a.to_string(), w));
        }
    }

    if !had_any {
        return Err("no graph: provide at least one edge or node (e.g. `a -> b`)".into());
    }

    // Sort each node's neighbor list by label for deterministic traversal,
    // de-duplicating parallel edges by keeping the minimum weight.
    for (_, list) in adj.iter_mut() {
        let mut best: BTreeMap<String, f64> = BTreeMap::new();
        for (nb, w) in list.iter() {
            best.entry(nb.clone())
                .and_modify(|e| {
                    if *w < *e {
                        *e = *w;
                    }
                })
                .or_insert(*w);
        }
        *list = best.into_iter().collect();
    }

    Ok(Graph {
        nodes,
        adj,
        directed,
    })
}

fn require_node(g: &Graph, label: &str, role: &str) -> Result<String, String> {
    let l = label.trim();
    if l.is_empty() {
        return Err(format!("{role} node is required for this algorithm"));
    }
    if !g.adj.contains_key(l) {
        return Err(format!(
            "{role} node {l:?} is not in the graph (nodes: {})",
            g.nodes.join(", ")
        ));
    }
    Ok(l.to_string())
}

fn bfs(g: &Graph, start: &str) -> Vec<String> {
    let mut order = Vec::new();
    let mut visited: BTreeSet<String> = BTreeSet::new();
    let mut q: VecDeque<String> = VecDeque::new();
    q.push_back(start.to_string());
    visited.insert(start.to_string());
    while let Some(n) = q.pop_front() {
        order.push(n.clone());
        for (nb, _) in g.neighbors(&n) {
            if visited.insert(nb.clone()) {
                q.push_back(nb.clone());
            }
        }
    }
    order
}

fn dfs(g: &Graph, start: &str) -> Vec<String> {
    let mut order = Vec::new();
    let mut visited: BTreeSet<String> = BTreeSet::new();
    // Iterative DFS with an explicit stack; push neighbors in reverse so the
    // smallest-label neighbor is visited first (matches the sorted adjacency).
    let mut stack = vec![start.to_string()];
    while let Some(n) = stack.pop() {
        if !visited.insert(n.clone()) {
            continue;
        }
        order.push(n.clone());
        let nbs = g.neighbors(&n);
        for (nb, _) in nbs.iter().rev() {
            if !visited.contains(nb) {
                stack.push(nb.clone());
            }
        }
    }
    order
}

/// Dijkstra shortest path from `start` to `end`. Works for non-negative weights
/// (unit weights when unweighted). Returns (path, total_distance) or None when
/// `end` is unreachable.
fn shortest_path(g: &Graph, start: &str, end: &str) -> Option<(Vec<String>, f64)> {
    // Min-heap via an ordered float wrapper.
    #[derive(PartialEq)]
    struct State {
        cost: f64,
        node: String,
    }
    impl Eq for State {}
    impl Ord for State {
        fn cmp(&self, other: &Self) -> std::cmp::Ordering {
            // Reverse for a min-heap; tie-break by node label for determinism.
            other
                .cost
                .partial_cmp(&self.cost)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| other.node.cmp(&self.node))
        }
    }
    impl PartialOrd for State {
        fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
            Some(self.cmp(other))
        }
    }

    let mut dist: BTreeMap<String, f64> = BTreeMap::new();
    let mut prev: BTreeMap<String, String> = BTreeMap::new();
    let mut heap = BinaryHeap::new();
    dist.insert(start.to_string(), 0.0);
    heap.push(State {
        cost: 0.0,
        node: start.to_string(),
    });

    while let Some(State { cost, node }) = heap.pop() {
        if node == end {
            break;
        }
        if cost > *dist.get(&node).unwrap_or(&f64::INFINITY) {
            continue;
        }
        for (nb, w) in g.neighbors(&node) {
            let next = cost + w;
            if next < *dist.get(nb).unwrap_or(&f64::INFINITY) {
                dist.insert(nb.clone(), next);
                prev.insert(nb.clone(), node.clone());
                heap.push(State {
                    cost: next,
                    node: nb.clone(),
                });
            }
        }
    }

    let total = *dist.get(end)?;
    // Reconstruct the path by walking `prev` back from `end`.
    let mut path = vec![end.to_string()];
    let mut cur = end.to_string();
    while cur != start {
        let p = prev.get(&cur)?.clone();
        path.push(p.clone());
        cur = p;
    }
    path.reverse();
    Some((path, total))
}

/// Kahn's algorithm topological sort. Returns Ok(order) for a DAG, or Err with
/// the offending nodes if a cycle prevents a complete ordering.
fn topological_sort(g: &Graph) -> Result<Vec<String>, String> {
    if !g.directed {
        return Err("topological sort requires a directed graph (set directed = true)".into());
    }
    let mut indeg: BTreeMap<String, usize> = BTreeMap::new();
    for n in g.adj.keys() {
        indeg.entry(n.clone()).or_insert(0);
    }
    for (_, nbs) in g.adj.iter() {
        for (nb, _) in nbs {
            *indeg.entry(nb.clone()).or_insert(0) += 1;
        }
    }
    // Seed the queue in node first-seen order among the zero-indegree nodes for
    // a stable, input-reflecting result.
    let mut queue: VecDeque<String> = VecDeque::new();
    for n in &g.nodes {
        if indeg.get(n).copied().unwrap_or(0) == 0 {
            queue.push_back(n.clone());
        }
    }
    let mut order = Vec::new();
    while let Some(n) = queue.pop_front() {
        order.push(n.clone());
        for (nb, _) in g.neighbors(&n) {
            let e = indeg.get_mut(nb).unwrap();
            *e -= 1;
            if *e == 0 {
                queue.push_back(nb.clone());
            }
        }
    }
    if order.len() != indeg.len() {
        let stuck: Vec<String> = indeg
            .iter()
            .filter(|(_, &d)| d > 0)
            .map(|(n, _)| n.clone())
            .collect();
        return Err(format!(
            "graph has a cycle — cannot produce a topological order (nodes in cycle(s): {})",
            stuck.join(", ")
        ));
    }
    Ok(order)
}

/// Detect a cycle. For a directed graph uses DFS colors and returns one example
/// cycle path. For an undirected graph uses DFS ignoring the parent edge.
fn find_cycle(g: &Graph) -> Option<Vec<String>> {
    if g.directed {
        // 0 = white, 1 = gray (on stack), 2 = black (done).
        let mut color: BTreeMap<String, u8> = BTreeMap::new();
        let mut stack: Vec<String> = Vec::new();
        for start in &g.nodes {
            if color.get(start).copied().unwrap_or(0) == 0 {
                if let Some(c) = dfs_cycle_directed(g, start, &mut color, &mut stack) {
                    return Some(c);
                }
            }
        }
        None
    } else {
        let mut visited: BTreeSet<String> = BTreeSet::new();
        for start in &g.nodes {
            if !visited.contains(start) {
                if let Some(c) = dfs_cycle_undirected(g, start, "", &mut visited) {
                    return Some(c);
                }
            }
        }
        None
    }
}

fn dfs_cycle_directed(
    g: &Graph,
    n: &str,
    color: &mut BTreeMap<String, u8>,
    stack: &mut Vec<String>,
) -> Option<Vec<String>> {
    color.insert(n.to_string(), 1);
    stack.push(n.to_string());
    for (nb, _) in g.neighbors(n) {
        match color.get(nb).copied().unwrap_or(0) {
            0 => {
                if let Some(c) = dfs_cycle_directed(g, nb, color, stack) {
                    return Some(c);
                }
            }
            1 => {
                // Back edge: extract the cycle from the stack.
                let idx = stack.iter().position(|x| x == nb).unwrap();
                let mut cycle: Vec<String> = stack[idx..].to_vec();
                cycle.push(nb.to_string());
                return Some(cycle);
            }
            _ => {}
        }
    }
    stack.pop();
    color.insert(n.to_string(), 2);
    None
}

fn dfs_cycle_undirected(
    g: &Graph,
    n: &str,
    parent: &str,
    visited: &mut BTreeSet<String>,
) -> Option<Vec<String>> {
    visited.insert(n.to_string());
    for (nb, _) in g.neighbors(n) {
        if !visited.contains(nb) {
            if let Some(c) = dfs_cycle_undirected(g, nb, n, visited) {
                return Some(c);
            }
        } else if nb != parent {
            // Found a back edge to an already-visited non-parent node.
            return Some(vec![n.to_string(), nb.to_string()]);
        }
    }
    None
}

/// Run a graph algorithm and produce a human-readable report.
///
/// - `edges`: the edge list (see `parse_graph` for formats).
/// - `algorithm`: `"bfs"` | `"dfs"` | `"shortest-path"` | `"topological-sort"` |
///   `"cycle-detection"` (blank → `"bfs"`).
/// - `directed`: treat edges as directed (default false).
/// - `weighted`: parse a trailing number on each edge as its weight (default false).
/// - `start`: start node for bfs/dfs/shortest-path.
/// - `end`: end node for shortest-path.
pub fn analyze(
    edges: &str,
    algorithm: &str,
    directed: bool,
    weighted: bool,
    start: &str,
    end: &str,
) -> Result<String, String> {
    let algo = match algorithm.trim() {
        "" | "bfs" => "bfs",
        "dfs" => "dfs",
        "shortest-path" | "shortest_path" | "dijkstra" => "shortest-path",
        "topological-sort" | "topological_sort" | "topo" => "topological-sort",
        "cycle-detection" | "cycle_detection" | "cycle" => "cycle-detection",
        other => {
            return Err(format!(
                "invalid algorithm {other:?}: expected bfs, dfs, shortest-path, topological-sort, or cycle-detection"
            ))
        }
    };

    let g = parse_graph(edges, directed, weighted)?;

    let kind = if g.directed { "directed" } else { "undirected" };
    let mut out = String::new();

    match algo {
        "bfs" => {
            let s = require_node(&g, start, "start")?;
            let order = bfs(&g, &s);
            out.push_str(&format!("Breadth-first search from {s:?} ({kind} graph):\n"));
            out.push_str(&format!("  visit order: {}\n", order.join(" → ")));
            out.push_str(&format!("  reached {} of {} nodes", order.len(), g.nodes.len()));
        }
        "dfs" => {
            let s = require_node(&g, start, "start")?;
            let order = dfs(&g, &s);
            out.push_str(&format!("Depth-first search from {s:?} ({kind} graph):\n"));
            out.push_str(&format!("  visit order: {}\n", order.join(" → ")));
            out.push_str(&format!("  reached {} of {} nodes", order.len(), g.nodes.len()));
        }
        "shortest-path" => {
            let s = require_node(&g, start, "start")?;
            let e = require_node(&g, end, "end")?;
            out.push_str(&format!(
                "Shortest path from {s:?} to {e:?} ({kind}, {} graph):\n",
                if weighted { "weighted" } else { "unweighted" }
            ));
            match shortest_path(&g, &s, &e) {
                Some((path, total)) => {
                    out.push_str(&format!("  path: {}\n", path.join(" → ")));
                    // Show the distance as an integer when it's whole.
                    if (total - total.round()).abs() < 1e-9 {
                        out.push_str(&format!(
                            "  total distance: {} ({} hop(s))",
                            total.round() as i64,
                            path.len() - 1
                        ));
                    } else {
                        out.push_str(&format!(
                            "  total distance: {total} ({} hop(s))",
                            path.len() - 1
                        ));
                    }
                }
                None => out.push_str(&format!("  no path exists from {s:?} to {e:?}")),
            }
        }
        "topological-sort" => match topological_sort(&g) {
            Ok(order) => {
                out.push_str("Topological sort (directed acyclic graph):\n");
                out.push_str(&format!("  order: {}", order.join(" → ")));
            }
            Err(e) => return Err(e),
        },
        "cycle-detection" => {
            out.push_str(&format!("Cycle detection ({kind} graph):\n"));
            match find_cycle(&g) {
                Some(cycle) => {
                    out.push_str("  result: a cycle was found\n");
                    out.push_str(&format!("  example cycle: {}", cycle.join(" → ")));
                }
                None => out.push_str("  result: no cycle — the graph is acyclic"),
            }
        }
        _ => unreachable!(),
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bfs_visits_in_breadth_order() {
        let r = analyze("a - b\na - c\nb - d", "bfs", false, false, "a", "").unwrap();
        assert!(r.contains("a → b → c → d"), "got: {r}");
        assert!(r.contains("reached 4 of 4"), "got: {r}");
    }

    #[test]
    fn dfs_goes_deep_first() {
        let r = analyze("a - b\na - c\nb - d", "dfs", false, false, "a", "").unwrap();
        // From a, smallest neighbor b first, then b's child d, then back to c.
        assert!(r.contains("a → b → d → c"), "got: {r}");
    }

    #[test]
    fn shortest_path_weighted() {
        // a->b (1), a->c (5), b->c (1): cheapest a..c is a-b-c = 2, not a-c = 5.
        let r = analyze(
            "a -> b : 1\na -> c : 5\nb -> c : 1",
            "shortest-path",
            true,
            true,
            "a",
            "c",
        )
        .unwrap();
        assert!(r.contains("a → b → c"), "got: {r}");
        assert!(r.contains("total distance: 2"), "got: {r}");
    }

    #[test]
    fn shortest_path_unreachable() {
        let r = analyze("a -> b\nc -> d", "shortest-path", true, false, "a", "d").unwrap();
        assert!(r.contains("no path exists"), "got: {r}");
    }

    #[test]
    fn topological_sort_orders_dag() {
        let r = analyze(
            "a -> b\na -> c\nb -> d\nc -> d",
            "topological-sort",
            true,
            false,
            "",
            "",
        )
        .unwrap();
        assert!(r.contains("order: a → "), "got: {r}");
        // d must come last.
        assert!(r.trim_end().ends_with("→ d"), "got: {r}");
    }

    #[test]
    fn topological_sort_rejects_cycle() {
        let err = analyze("a -> b\nb -> a", "topological-sort", true, false, "", "")
            .unwrap_err();
        assert!(err.contains("cycle"), "got: {err}");
    }

    #[test]
    fn topological_sort_requires_directed() {
        let err = analyze("a - b", "topological-sort", false, false, "", "").unwrap_err();
        assert!(err.contains("directed"), "got: {err}");
    }

    #[test]
    fn cycle_detection_directed_finds_cycle() {
        let r = analyze("a -> b\nb -> c\nc -> a", "cycle-detection", true, false, "", "")
            .unwrap();
        assert!(r.contains("a cycle was found"), "got: {r}");
    }

    #[test]
    fn cycle_detection_directed_acyclic() {
        let r = analyze("a -> b\nb -> c", "cycle-detection", true, false, "", "").unwrap();
        assert!(r.contains("no cycle"), "got: {r}");
    }

    #[test]
    fn cycle_detection_undirected() {
        let r = analyze("a - b\nb - c\nc - a", "cycle-detection", false, false, "", "")
            .unwrap();
        assert!(r.contains("a cycle was found"), "got: {r}");
        // A simple undirected tree has no cycle.
        let r2 = analyze("a - b\nb - c", "cycle-detection", false, false, "", "").unwrap();
        assert!(r2.contains("no cycle"), "got: {r2}");
    }

    #[test]
    fn rejects_unknown_algorithm() {
        let err = analyze("a - b", "spanning-tree", false, false, "a", "").unwrap_err();
        assert!(err.contains("invalid algorithm"), "got: {err}");
    }

    #[test]
    fn rejects_missing_start_node() {
        let err = analyze("a - b", "bfs", false, false, "z", "").unwrap_err();
        assert!(err.contains("not in the graph"), "got: {err}");
    }

    #[test]
    fn rejects_empty_graph() {
        let err = analyze("\n  \n# comment", "bfs", false, false, "a", "").unwrap_err();
        assert!(err.contains("no graph"), "got: {err}");
    }

    #[test]
    fn rejects_negative_weight() {
        let err = analyze("a -> b : -2", "shortest-path", true, true, "a", "b").unwrap_err();
        assert!(err.contains("negative weight"), "got: {err}");
    }

    #[test]
    fn parses_multiple_separators() {
        // comma, arrow, and plain space all parse as edges.
        let r = analyze("a,b\nb -> c\nc d", "bfs", false, false, "a", "").unwrap();
        assert!(r.contains("a → b → c → d"), "got: {r}");
    }

    #[test]
    fn isolated_node_is_included() {
        let r = analyze("a - b\nz", "bfs", false, false, "z", "").unwrap();
        assert!(r.contains("reached 1 of 3"), "got: {r}");
    }
}
