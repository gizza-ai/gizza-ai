//! Browser-facing wasm-bindgen wrapper for /tools/graph-algorithms/.
//! Compiled with wasm-pack for the standalone /tools/graph-algorithms/ page.
use wasm_bindgen::prelude::*;

/// Run a graph algorithm on an edge list.
///
/// The standalone page passes every field value as a string, so the boolean
/// params arrive as strings and are parsed here:
/// - `edges`: the edge list, one edge per line.
/// - `algorithm`: bfs | dfs | shortest-path | topological-sort | cycle-detection
///   (blank → bfs).
/// - `directed` / `weighted`: `"true"`/`"1"`/`"yes"`/`"on"` → true, else false.
/// - `start` / `end`: node labels (used by bfs/dfs/shortest-path).
///
/// Throws a JS error string on invalid input (unknown algorithm, missing node,
/// negative weight, empty graph).
#[wasm_bindgen]
pub fn run(
    edges: &str,
    algorithm: &str,
    directed: &str,
    weighted: &str,
    start: &str,
    end: &str,
) -> Result<String, JsValue> {
    let truthy = |v: &str| {
        matches!(
            v.trim().to_ascii_lowercase().as_str(),
            "true" | "1" | "yes" | "on"
        )
    };
    gizza_ai_graph_algorithms_core::analyze(
        edges,
        algorithm,
        truthy(directed),
        truthy(weighted),
        start,
        end,
    )
    .map_err(|e| JsValue::from_str(&e))
}
