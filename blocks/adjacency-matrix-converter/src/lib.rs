//! gizza-ai/adjacency-matrix-converter — convert a graph between an edge list,
//! an adjacency matrix, and an incidence matrix. Chat skill block on the shared
//! tool abstraction; the chat schema is single-sourced from descriptor() (which
//! also drives the CLI). No host calls — runs entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default)]
    from: String,
    #[serde(default)]
    to: String,
    #[serde(default)]
    directed: bool,
    #[serde(default)]
    weighted: bool,
    #[serde(default)]
    power: i64,
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("The graph to convert. For from='edges': one edge per line as 'A B' (or 'A B 3' when weighted); endpoints are whitespace- or comma-separated; a single token declares an isolated vertex; '#' starts a comment. For from='adjacency': a square numeric matrix, one row per line, cells whitespace- or comma-separated; an optional header row/column of labels is auto-detected. For from='list': an adjacency list of neighbors. For from='incidence': a vertices×edges incidence matrix."),
        )
        .param(
            Param::enumv("from", ["auto", "edges", "adjacency", "list", "incidence"])
                .default("auto")
                .describe("Input format: 'auto' (default) auto-detects structure; 'edges' an edge list; 'adjacency' a square adjacency matrix; 'list' an adjacency list; 'incidence' an incidence matrix."),
        )
        .param(
            Param::enumv("to", ["adjacency", "incidence", "edges", "list", "degree", "laplacian", "stats", "power"])
                .default("adjacency")
                .describe("Output format: 'adjacency' (default) a labelled adjacency matrix; 'incidence' a vertices×edges incidence matrix (directed uses -1 tail / +1 head); 'edges' a normalized edge list; 'list' an adjacency list ('A: B C'); 'degree' the diagonal degree matrix; 'laplacian' the graph Laplacian L=D-A (undirected only); 'stats' analytical graph metrics; 'power' walk-count matrix raised to power k."),
        )
        .param(
            Param::boolean("directed")
                .default(false)
                .describe("Treat the graph as directed: 'A B' does NOT imply 'B A'. Default false (undirected)."),
        )
        .param(
            Param::boolean("weighted")
                .default(false)
                .describe("Read/emit edge weights: the 3rd token of an edge line, or the matrix cell value. Default false (every present edge is 1)."),
        )
        .param(
            Param::integer("power")
                .default(2)
                .describe("The power k to raise the adjacency matrix to when to='power' (calculates walk counts of length k). Range 1 to 10."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/adjacency-matrix-converter",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert a graph between an edge list, an adjacency matrix, and an incidence matrix.",
    skill(
        description = "Convert a graph between an edge list, an adjacency matrix, an adjacency list, an incidence matrix, a degree matrix, and the graph Laplacian. Set from='edges' (default, one 'A B' edge per line, optional 3rd weight token) or from='adjacency' (a square numeric matrix with auto-detected label header). Set to='adjacency' (default), 'incidence' (vertices×edges; directed uses -1 for the tail and +1 for the head), 'edges', 'list' (adjacency list), 'degree' (diagonal degree matrix), or 'laplacian' (L=D-A, undirected only). Use directed=true so 'A B' is one-way, and weighted=true to read/emit edge weights (otherwise every edge is 1). Matrices are tab-separated with a label row and column.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "adjacency-matrix-converter", |a: Args| {
            let p = if a.power == 0 { 2 } else { a.power };
            gizza_ai_adjacency_matrix_converter_core::convert(
                &a.input, &a.from, &a.to, a.directed, a.weighted, p,
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
    /// schema, so any future change to the LLM-facing API is intentional.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "input": { "type": "string", "description": "The graph to convert. For from='edges': one edge per line as 'A B' (or 'A B 3' when weighted); endpoints are whitespace- or comma-separated; a single token declares an isolated vertex; '#' starts a comment. For from='adjacency': a square numeric matrix, one row per line, cells whitespace- or comma-separated; an optional header row/column of labels is auto-detected. For from='list': an adjacency list of neighbors. For from='incidence': a vertices×edges incidence matrix." },
                    "from": { "type": "string", "enum": ["auto", "edges", "adjacency", "list", "incidence"], "default": "auto", "description": "Input format: 'auto' (default) auto-detects structure; 'edges' an edge list; 'adjacency' a square adjacency matrix; 'list' an adjacency list; 'incidence' an incidence matrix." },
                    "to": { "type": "string", "enum": ["adjacency", "incidence", "edges", "list", "degree", "laplacian", "stats", "power"], "default": "adjacency", "description": "Output format: 'adjacency' (default) a labelled adjacency matrix; 'incidence' a vertices×edges incidence matrix (directed uses -1 tail / +1 head); 'edges' a normalized edge list; 'list' an adjacency list ('A: B C'); 'degree' the diagonal degree matrix; 'laplacian' the graph Laplacian L=D-A (undirected only); 'stats' analytical graph metrics; 'power' walk-count matrix raised to power k." },
                    "directed": { "type": "boolean", "default": false, "description": "Treat the graph as directed: 'A B' does NOT imply 'B A'. Default false (undirected)." },
                    "weighted": { "type": "boolean", "default": false, "description": "Read/emit edge weights: the 3rd token of an edge line, or the matrix cell value. Default false (every present edge is 1)." },
                    "power": { "type": "integer", "default": 2, "description": "The power k to raise the adjacency matrix to when to='power' (calculates walk counts of length k). Range 1 to 10." }
                },
                "required": ["input"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
