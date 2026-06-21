//! gizza-ai/dot-to-svg — render Graphviz DOT diagram source into a standalone
//! SVG, with no Graphviz install. Pure-Rust layout engine, so it runs on every
//! backend (incl. the chat Service Worker). The chat schema is single-sourced
//! from descriptor() (which also drives the CLI); handle() delegates to
//! block_utils::run_skill. The SVG markup is returned as text (like svg-optimize).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_dot_to_svg_core::{render, Options};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    dot: String,
    #[serde(default)]
    dark_mode: bool,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("dot").required().describe(
                "Graphviz DOT source to render, e.g. 'digraph { a -> b; b -> c; a -> c; }'. \
                 Supports directed (digraph) and undirected (graph) graphs, node/edge labels, \
                 and chained edges.",
            ),
        )
        .param(
            Param::boolean("dark_mode")
                .default(false)
                .describe(
                    "Recolor the SVG for a dark background: light text, edges and node strokes \
                     on a transparent canvas (default false).",
                ),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/dot-to-svg",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Render Graphviz DOT to SVG",
    skill(
        description = "Render Graphviz DOT diagram source into a standalone, scalable SVG — no Graphviz install needed. Pass `dot` as DOT source, e.g. 'digraph { a -> b; b -> c; a -> c; }' for a directed graph, or 'graph { x -- y; y -- z; }' for an undirected one; node/edge labels and chained edges are supported. Optional `dark_mode` recolors the SVG (light text/edges/strokes) for a dark background. Returns the SVG markup as text. Runs locally on the device.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "dot-to-svg", |a: Args| {
            render(&a.dot, &Options { dark_mode: a.dark_mode }).map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "dot": { "type": "string", "description": "Graphviz DOT source to render, e.g. 'digraph { a -> b; b -> c; a -> c; }'. Supports directed (digraph) and undirected (graph) graphs, node/edge labels, and chained edges." },
                    "dark_mode": { "type": "boolean", "default": false, "description": "Recolor the SVG for a dark background: light text, edges and node strokes on a transparent canvas (default false)." }
                },
                "required": ["dot"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
