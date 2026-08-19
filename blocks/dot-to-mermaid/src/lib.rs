//! gizza-ai/dot-to-mermaid — translate Graphviz DOT source into Mermaid
//! flowchart syntax. Pure compute (no I/O), so the same core runs in chat, on
//! the CLI and in the browser page. The chat schema is single-sourced from
//! descriptor() (which also drives the CLI); handle() delegates to
//! block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_dot_to_mermaid_core::{convert, Options};
use serde::Deserialize;
use wafer_sdk::*;

fn yes() -> bool {
    true
}

#[derive(Deserialize)]
struct Args {
    dot: String,
    #[serde(default)]
    direction: Option<String>,
    #[serde(default = "yes")]
    shapes: bool,
    #[serde(default = "yes")]
    edge_labels: bool,
    #[serde(default = "yes")]
    link_styles: bool,
    #[serde(default = "yes")]
    subgraphs: bool,
    #[serde(default = "yes")]
    colors: bool,
    #[serde(default = "yes")]
    warnings: bool,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    fence: bool,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("dot").required().describe(
            "Graphviz DOT source to translate, e.g. 'digraph { a -> b [label=\"go\"]; }'. \
             Accepts digraph and graph, chained edges (a -> b -> c), attribute lists, \
             node/edge/graph defaults, nested subgraphs and cluster_* clusters, ports, \
             comments, quoted and HTML-style labels.",
        ))
        .param(
            Param::enumv("direction", ["auto", "TD", "LR", "BT", "RL"])
                .default("auto")
                .describe(
                    "Mermaid flowchart direction. 'auto' (default) follows the graph's own \
                     rankdir (TB->TD, LR, BT, RL) and falls back to TD when there is none; \
                     TD/LR/BT/RL force a direction.",
                ),
        )
        .param(Param::boolean("shapes").default(true).describe(
            "Map Graphviz shape= attributes onto Mermaid node shapes (box->[], ellipse->(), \
             circle->(()), doublecircle->((())), diamond->{}, hexagon->{{}}, cylinder->[()], \
             component/box3d->[[]], parallelogram->[//], trapezium->[/\\]). Default true; \
             false emits every node as a plain rectangle.",
        ))
        .param(Param::boolean("edge_labels").default(true).describe(
            "Carry Graphviz edge label/xlabel text onto the Mermaid link (a -->|go| b). \
             Default true; false emits unlabelled links.",
        ))
        .param(Param::boolean("link_styles").default(true).describe(
            "Map Graphviz edge style=/dir= onto Mermaid link types: dashed/dotted -> -.->, \
             bold -> ==>, invis -> ~~~, dir=none -> ---, dir=both -> <-->, dir=back reverses \
             the edge. Default true; false emits plain --> (or --- for undirected graphs).",
        ))
        .param(Param::boolean("subgraphs").default(true).describe(
            "Translate 'subgraph cluster_*' blocks (including nested ones) into Mermaid \
             subgraph/end blocks, using the cluster's label= as the block title. Default \
             true; false flattens every cluster to top-level nodes.",
        ))
        .param(Param::boolean("colors").default(true).describe(
            "Emit Mermaid 'style <node> fill:…,stroke:…,color:…' and 'linkStyle <n> stroke:…' \
             lines from Graphviz fillcolor/color/fontcolor/penwidth. Default true. Graphviz \
             color lists ('red:blue') and colorscheme references ('/set19/3') are skipped.",
        ))
        .param(Param::boolean("warnings").default(true).describe(
            "Append '%%' Mermaid comment notes listing DOT features that have no Mermaid \
             equivalent (unknown shapes, ports, color lists, flattened clusters). Default \
             true; false returns diagram source only.",
        ))
        .param(Param::string("title").describe(
            "Optional diagram title, emitted as Mermaid YAML front matter (---\\ntitle: …\\n---). \
             Leave empty to use the graph's own label= attribute, if it has one.",
        ))
        .param(Param::boolean("fence").default(false).describe(
            "Wrap the result in a ```mermaid code fence, ready to paste into a Markdown or \
             README file. Default false (bare diagram source).",
        ))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/dot-to-mermaid",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert Graphviz DOT to Mermaid flowchart syntax",
    skill(
        description = "Convert a Graphviz DOT graph into Mermaid flowchart syntax. Pass `dot` as DOT source, e.g. 'digraph { rankdir=LR; a [label=\"Start\", shape=circle]; a -> b [label=\"go\"]; }'. Nodes, chained edges, labels, shapes, edge styles, cluster subgraphs and colors are mapped to their Mermaid equivalents; `direction` picks the flowchart direction (auto follows rankdir); `shapes`, `edge_labels`, `link_styles`, `subgraphs`, `colors` and `warnings` toggle each mapping; `title` sets YAML front matter; `fence` wraps the output in a ```mermaid code fence. Returns Mermaid source as text, with `%%` notes for DOT features Mermaid cannot express. Runs locally on the device.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "dot-to-mermaid", |a: Args| {
            let opts = Options {
                direction: a.direction.unwrap_or_else(|| "auto".into()),
                shapes: a.shapes,
                edge_labels: a.edge_labels,
                link_styles: a.link_styles,
                subgraphs: a.subgraphs,
                colors: a.colors,
                warnings: a.warnings,
                title: a.title.unwrap_or_default(),
                fence: a.fence,
            };
            convert(&a.dot, &opts).map_err(SkillError::InvalidArgs)
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
                    "dot": { "type": "string", "description": "Graphviz DOT source to translate, e.g. 'digraph { a -> b [label=\"go\"]; }'. Accepts digraph and graph, chained edges (a -> b -> c), attribute lists, node/edge/graph defaults, nested subgraphs and cluster_* clusters, ports, comments, quoted and HTML-style labels." },
                    "direction": { "type": "string", "enum": ["auto", "TD", "LR", "BT", "RL"], "default": "auto", "description": "Mermaid flowchart direction. 'auto' (default) follows the graph's own rankdir (TB->TD, LR, BT, RL) and falls back to TD when there is none; TD/LR/BT/RL force a direction." },
                    "shapes": { "type": "boolean", "default": true, "description": "Map Graphviz shape= attributes onto Mermaid node shapes (box->[], ellipse->(), circle->(()), doublecircle->((())), diamond->{}, hexagon->{{}}, cylinder->[()], component/box3d->[[]], parallelogram->[//], trapezium->[/\\]). Default true; false emits every node as a plain rectangle." },
                    "edge_labels": { "type": "boolean", "default": true, "description": "Carry Graphviz edge label/xlabel text onto the Mermaid link (a -->|go| b). Default true; false emits unlabelled links." },
                    "link_styles": { "type": "boolean", "default": true, "description": "Map Graphviz edge style=/dir= onto Mermaid link types: dashed/dotted -> -.->, bold -> ==>, invis -> ~~~, dir=none -> ---, dir=both -> <-->, dir=back reverses the edge. Default true; false emits plain --> (or --- for undirected graphs)." },
                    "subgraphs": { "type": "boolean", "default": true, "description": "Translate 'subgraph cluster_*' blocks (including nested ones) into Mermaid subgraph/end blocks, using the cluster's label= as the block title. Default true; false flattens every cluster to top-level nodes." },
                    "colors": { "type": "boolean", "default": true, "description": "Emit Mermaid 'style <node> fill:…,stroke:…,color:…' and 'linkStyle <n> stroke:…' lines from Graphviz fillcolor/color/fontcolor/penwidth. Default true. Graphviz color lists ('red:blue') and colorscheme references ('/set19/3') are skipped." },
                    "warnings": { "type": "boolean", "default": true, "description": "Append '%%' Mermaid comment notes listing DOT features that have no Mermaid equivalent (unknown shapes, ports, color lists, flattened clusters). Default true; false returns diagram source only." },
                    "title": { "type": "string", "description": "Optional diagram title, emitted as Mermaid YAML front matter (---\\ntitle: …\\n---). Leave empty to use the graph's own label= attribute, if it has one." },
                    "fence": { "type": "boolean", "default": false, "description": "Wrap the result in a ```mermaid code fence, ready to paste into a Markdown or README file. Default false (bare diagram source)." }
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
