//! gizza-ai/mermaid-from-data — convert structured node/edge rows into Mermaid
//! source for flowcharts, class diagrams or ER diagrams. Chat schema is
//! single-sourced from descriptor() (which also drives the CLI); handle()
//! delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    edges: String,
    #[serde(default)]
    nodes: String,
    #[serde(default = "default_diagram")]
    diagram: String,
    #[serde(default = "default_direction")]
    direction: String,
    #[serde(default = "default_row_mode")]
    row_mode: String,
    #[serde(default = "default_delimiter")]
    delimiter: String,
    #[serde(default = "default_node_shape")]
    node_shape: String,
    #[serde(default = "default_edge_style")]
    edge_style: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    fence: bool,
}

fn default_diagram() -> String {
    "flowchart".into()
}
fn default_direction() -> String {
    "TD".into()
}
fn default_row_mode() -> String {
    "pair".into()
}
fn default_delimiter() -> String {
    "auto".into()
}
fn default_node_shape() -> String {
    "rect".into()
}
fn default_edge_style() -> String {
    "arrow".into()
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("edges").required().describe("Edges or paths to turn into Mermaid source. Write one edge per line as `from -> to : label`, `from,to,label,style`, `from|to|label|style`, or use chain mode where every value in a row becomes a node and consecutive values are linked. Blank lines plus # and // comments are ignored. Up to 1000 edges and 2 MB."))
        .param(Param::string("nodes").default("").describe("Optional node declarations, one per line: `id|label|shape|group` for flowcharts, `Class | +field; +method()` for class diagrams, or `ENTITY | string id PK; string name` for ER diagrams. Leave blank to discover nodes from edges."))
        .param(Param::enumv("diagram", ["flowchart", "class", "er"]).default("flowchart").describe("Diagram syntax to emit. `flowchart` creates a Mermaid flowchart, `class` creates a classDiagram, and `er` creates an erDiagram."))
        .param(Param::enumv("direction", ["TD", "TB", "LR", "BT", "RL"]).default("TD").describe("Diagram direction. TD/TB draw top-to-bottom, LR left-to-right, BT bottom-to-top, and RL right-to-left. ER diagrams render TD as Mermaid direction TB."))
        .param(Param::enumv("row_mode", ["pair", "chain"]).default("pair").describe("How delimited edge rows are interpreted. `pair` reads from/to plus optional label/style columns. `chain` links every adjacent value in a row, useful for paths like `Draft,Review,Ship`."))
        .param(Param::enumv("delimiter", ["auto", "pipe", "comma", "tab"]).default("auto").describe("Delimiter for table-style rows. `auto` detects pipe, tab, then comma; arrow lines such as `A -> B : label` work regardless."))
        .param(Param::enumv("node_shape", ["rect", "round", "stadium", "subroutine", "cylinder", "circle", "diamond", "hexagon", "parallelogram", "trapezoid"]).default("rect").describe("Default flowchart node shape for discovered nodes. Node declarations can override this per node. Ignored for class and ER diagrams."))
        .param(Param::enumv("edge_style", ["arrow", "open", "dotted", "thick", "bidirectional"]).default("arrow").describe("Default flowchart edge style for rows without a style column. Individual rows can use arrow/open/dotted/thick/bidirectional. Class and ER diagrams interpret the style column as a relationship type."))
        .param(Param::string("title").default("").describe("Optional Mermaid front-matter title placed before the diagram. Leave blank for no title."))
        .param(Param::boolean("fence").default(false).describe("Wrap the Mermaid source in a ```mermaid fenced code block for Markdown paste targets. Default false."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/mermaid-from-data",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Generate Mermaid diagram source from structured nodes and edges",
    skill(
        description = "Generate Mermaid diagram source from structured node and edge data. Paste arrow lines such as `Start -> Build : work`, delimited rows such as `from,to,label,style`, or chain rows such as `Draft,Review,Ship`; optional node declarations can set labels, flowchart shapes, subgraph groups, class members or ER attributes. The tool emits flowchart, classDiagram or erDiagram syntax, supports diagram directions, flowchart edge styles, class/ER relationship styles, optional front-matter title and an optional fenced Markdown block. It validates unknown styles, caps input to 2 MB, 500 nodes and 1000 edges, sanitizes Mermaid identifiers, and returns plain text Mermaid source for local rendering in any Mermaid-compatible editor.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "mermaid-from-data", |a: Args| {
            gizza_ai_mermaid_from_data_core::generate(
                &a.edges,
                &a.nodes,
                &a.diagram,
                &a.direction,
                &a.row_mode,
                &a.delimiter,
                &a.node_shape,
                &a.edge_style,
                &a.title,
                a.fence,
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

    /// Drift guard: the authored chat/CLI schema must stay identical to the one
    /// derived from descriptor(). Update BOTH sides together, on purpose.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "edges": { "type": "string", "description": "Edges or paths to turn into Mermaid source. Write one edge per line as `from -> to : label`, `from,to,label,style`, `from|to|label|style`, or use chain mode where every value in a row becomes a node and consecutive values are linked. Blank lines plus # and // comments are ignored. Up to 1000 edges and 2 MB." },
                    "nodes": { "type": "string", "default": "", "description": "Optional node declarations, one per line: `id|label|shape|group` for flowcharts, `Class | +field; +method()` for class diagrams, or `ENTITY | string id PK; string name` for ER diagrams. Leave blank to discover nodes from edges." },
                    "diagram": { "type": "string", "enum": ["flowchart", "class", "er"], "default": "flowchart", "description": "Diagram syntax to emit. `flowchart` creates a Mermaid flowchart, `class` creates a classDiagram, and `er` creates an erDiagram." },
                    "direction": { "type": "string", "enum": ["TD", "TB", "LR", "BT", "RL"], "default": "TD", "description": "Diagram direction. TD/TB draw top-to-bottom, LR left-to-right, BT bottom-to-top, and RL right-to-left. ER diagrams render TD as Mermaid direction TB." },
                    "row_mode": { "type": "string", "enum": ["pair", "chain"], "default": "pair", "description": "How delimited edge rows are interpreted. `pair` reads from/to plus optional label/style columns. `chain` links every adjacent value in a row, useful for paths like `Draft,Review,Ship`." },
                    "delimiter": { "type": "string", "enum": ["auto", "pipe", "comma", "tab"], "default": "auto", "description": "Delimiter for table-style rows. `auto` detects pipe, tab, then comma; arrow lines such as `A -> B : label` work regardless." },
                    "node_shape": { "type": "string", "enum": ["rect", "round", "stadium", "subroutine", "cylinder", "circle", "diamond", "hexagon", "parallelogram", "trapezoid"], "default": "rect", "description": "Default flowchart node shape for discovered nodes. Node declarations can override this per node. Ignored for class and ER diagrams." },
                    "edge_style": { "type": "string", "enum": ["arrow", "open", "dotted", "thick", "bidirectional"], "default": "arrow", "description": "Default flowchart edge style for rows without a style column. Individual rows can use arrow/open/dotted/thick/bidirectional. Class and ER diagrams interpret the style column as a relationship type." },
                    "title": { "type": "string", "default": "", "description": "Optional Mermaid front-matter title placed before the diagram. Leave blank for no title." },
                    "fence": { "type": "boolean", "default": false, "description": "Wrap the Mermaid source in a ```mermaid fenced code block for Markdown paste targets. Default false." }
                },
                "required": ["edges"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(authored, derived);
    }
}
