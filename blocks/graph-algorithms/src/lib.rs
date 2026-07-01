//! gizza-ai/graph-algorithms — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    edges: String,
    #[serde(default)]
    algorithm: String,
    #[serde(default)]
    directed: bool,
    #[serde(default)]
    weighted: bool,
    #[serde(default)]
    start: String,
    #[serde(default)]
    end: String,
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("edges")
                .required()
                .describe("The graph as an edge list, one edge per line. Use `a -> b`, `a - b`, `a b`, or `a,b` for an edge; add a trailing number for a weight (`a -> b : 3`); a bare label on its own line is an isolated node. Blank lines and `#` comments are ignored."),
        )
        .param(
            Param::enumv(
                "algorithm",
                ["bfs", "dfs", "shortest-path", "topological-sort", "cycle-detection"],
            )
            .default("bfs")
            .describe("Which algorithm to run: 'bfs' (breadth-first traversal), 'dfs' (depth-first traversal), 'shortest-path' (Dijkstra / unit-weight BFS from start to end), 'topological-sort' (linear order of a directed acyclic graph), or 'cycle-detection' (find a cycle)."),
        )
        .param(
            Param::boolean("directed")
                .default(false)
                .describe("Treat edges as directed (a → b only). Default false (undirected). Topological sort requires directed = true."),
        )
        .param(
            Param::boolean("weighted")
                .default(false)
                .describe("Parse a trailing number on each edge as its weight, used by shortest-path. Default false (every edge has weight 1). Weights must be non-negative."),
        )
        .param(
            Param::string("start")
                .describe("Start node for bfs, dfs, and shortest-path."),
        )
        .param(
            Param::string("end")
                .describe("End (target) node for shortest-path."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/graph-algorithms",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Run BFS, DFS, shortest path, topological sort, and cycle detection on a user-supplied graph.",
    skill(
        description = "Run a classic graph algorithm on a user-supplied edge list. Provide `edges` (one edge per line, e.g. `a -> b`, `a - b`, `a,b`, or weighted `a -> b : 3`), choose `algorithm` = bfs | dfs | shortest-path | topological-sort | cycle-detection, and set `directed`/`weighted` as needed. bfs/dfs/shortest-path need a `start` node; shortest-path also needs `end`. Returns a human-readable report (visit order, shortest path + distance, topological order, or an example cycle).",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "graph-algorithms", |a: Args| {
            gizza_ai_graph_algorithms_core::analyze(
                &a.edges,
                &a.algorithm,
                a.directed,
                a.weighted,
                &a.start,
                &a.end,
            )
            .map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema, so any change to the LLM-facing API is intentional and reviewed.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "edges": { "type": "string", "description": "The graph as an edge list, one edge per line. Use `a -> b`, `a - b`, `a b`, or `a,b` for an edge; add a trailing number for a weight (`a -> b : 3`); a bare label on its own line is an isolated node. Blank lines and `#` comments are ignored." },
                    "algorithm": { "type": "string", "enum": ["bfs", "dfs", "shortest-path", "topological-sort", "cycle-detection"], "default": "bfs", "description": "Which algorithm to run: 'bfs' (breadth-first traversal), 'dfs' (depth-first traversal), 'shortest-path' (Dijkstra / unit-weight BFS from start to end), 'topological-sort' (linear order of a directed acyclic graph), or 'cycle-detection' (find a cycle)." },
                    "directed": { "type": "boolean", "default": false, "description": "Treat edges as directed (a → b only). Default false (undirected). Topological sort requires directed = true." },
                    "weighted": { "type": "boolean", "default": false, "description": "Parse a trailing number on each edge as its weight, used by shortest-path. Default false (every edge has weight 1). Weights must be non-negative." },
                    "start": { "type": "string", "description": "Start node for bfs, dfs, and shortest-path." },
                    "end": { "type": "string", "description": "End (target) node for shortest-path." }
                },
                "required": ["edges"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
