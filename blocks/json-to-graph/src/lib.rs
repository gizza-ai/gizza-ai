//! gizza-ai/json-to-graph — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_json_to_graph_core::{to_graph, Direction, Format, Options};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    json: String,
    #[serde(default = "default_format")]
    format: String,
    #[serde(default = "default_direction")]
    direction: String,
    #[serde(default)]
    max_depth: u64,
    #[serde(default = "default_max_nodes")]
    max_nodes: u64,
    #[serde(default)]
    max_array_items: u64,
    #[serde(default = "default_include_values")]
    include_values: bool,
    #[serde(default = "default_value_max_len")]
    value_max_len: u64,
    #[serde(default)]
    show_types: bool,
}
fn default_format() -> String {
    "mermaid".to_string()
}
fn default_direction() -> String {
    "TD".to_string()
}
fn default_max_nodes() -> u64 {
    300
}
fn default_include_values() -> bool {
    true
}
fn default_value_max_len() -> u64 {
    40
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("json")
                .required()
                .describe("The JSON document to visualize (object, array, or any JSON value). Every key becomes a node and every parent/child relationship an edge, e.g. {\"users\":[{\"id\":1}]}."),
        )
        .param(
            Param::enumv("format", ["mermaid", "dot"])
                .default("mermaid")
                .describe("Diagram syntax to emit. 'mermaid' (default) returns `flowchart` source for Mermaid Live / GitHub / Notion; 'dot' returns a Graphviz `digraph` you can pipe into `dot -Tsvg`."),
        )
        .param(
            Param::enumv("direction", ["TD", "LR", "BT", "RL"])
                .default("TD")
                .describe("Layout direction: TD top-down (default), LR left-to-right (best for deep documents), BT bottom-up, RL right-to-left. Emitted as `flowchart <dir>` for Mermaid and `rankdir` for DOT (TD maps to TB)."),
        )
        .param(
            Param::integer("max_depth")
                .default(0)
                .min(0.0)
                .max(100.0)
                .describe("Stop expanding below this nesting level; the elided subtree becomes one '… N keys/items hidden' node. Root is level 0. 0 (default) expands every level."),
        )
        .param(
            Param::integer("max_nodes")
                .default(300)
                .min(1.0)
                .max(5000.0)
                .describe("Hard cap on nodes in the diagram. Nodes are added breadth-first, so the cap keeps the upper levels; a truncation comment is appended when it bites. Default 300."),
        )
        .param(
            Param::integer("max_array_items")
                .default(0)
                .min(0.0)
                .max(1000.0)
                .describe("Max elements drawn per array; the remainder collapses into one '… N more items' node. 0 (default) draws every element."),
        )
        .param(
            Param::boolean("include_values")
                .default(true)
                .describe("Show scalar values in leaf labels ('name: \"Ada\"'). Set false for a keys-only structure map. Default true."),
        )
        .param(
            Param::integer("value_max_len")
                .default(40)
                .min(0.0)
                .max(200.0)
                .describe("Truncate key names and string values longer than this many characters, appending '…'. 0 disables truncation. Default 40."),
        )
        .param(
            Param::boolean("show_types")
                .default(false)
                .describe("Annotate labels with type/size: objects get '{n}' keys, arrays '[n]' items, and (when include_values is false) scalars get their JSON type. Default false."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn options_from(a: &Args) -> Options {
    Options {
        format: Format::parse(&a.format),
        direction: Direction::parse(&a.direction),
        max_depth: a.max_depth as usize,
        max_nodes: a.max_nodes as usize,
        max_array_items: a.max_array_items as usize,
        include_values: a.include_values,
        value_max_len: a.value_max_len as usize,
        show_types: a.show_types,
    }
}

fn run(a: Args) -> Result<String, String> {
    to_graph(&a.json, &options_from(&a))
}

#[cfg(target_arch = "wasm32")]
struct JsonToGraph;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/json-to-graph",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Visualize a JSON document's structure as a Mermaid flowchart or Graphviz DOT graph",
    skill(
        description = "Turn a JSON document into a node-link graph you can render as a diagram. Each object key and array element becomes a node (objects are boxes, arrays stacked boxes, scalars ellipses) and each parent/child relationship an edge; leaf labels carry the scalar value. Output is diagram SOURCE, ready to paste into Mermaid Live, GitHub/GitLab markdown, Notion, or `dot -Tsvg`. Options: format (mermaid default, or dot for Graphviz), direction (TD default, LR, BT, RL), max_depth (collapse anything deeper into one '… hidden' node; 0 = all), max_nodes (breadth-first cap, default 300, adds a truncation comment), max_array_items (cap elements drawn per array; 0 = all), include_values (default true; false gives a keys-only structure map), value_max_len (truncate long keys/strings, default 40, 0 = off), show_types (annotate objects with {key count}, arrays with [length], scalars with their JSON type).",
        parameters = schema_json()
    ),
)]
impl JsonToGraph {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "json-to-graph", |a: Args| {
            run(a).map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(json: &str) -> Args {
        Args {
            json: json.to_string(),
            format: default_format(),
            direction: default_direction(),
            max_depth: 0,
            max_nodes: default_max_nodes(),
            max_array_items: 0,
            include_values: default_include_values(),
            value_max_len: default_value_max_len(),
            show_types: false,
        }
    }

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "json":            { "type": "string", "description": "The JSON document to visualize (object, array, or any JSON value). Every key becomes a node and every parent/child relationship an edge, e.g. {\"users\":[{\"id\":1}]}." },
                    "format":          { "type": "string", "enum": ["mermaid", "dot"], "default": "mermaid", "description": "Diagram syntax to emit. 'mermaid' (default) returns `flowchart` source for Mermaid Live / GitHub / Notion; 'dot' returns a Graphviz `digraph` you can pipe into `dot -Tsvg`." },
                    "direction":       { "type": "string", "enum": ["TD", "LR", "BT", "RL"], "default": "TD", "description": "Layout direction: TD top-down (default), LR left-to-right (best for deep documents), BT bottom-up, RL right-to-left. Emitted as `flowchart <dir>` for Mermaid and `rankdir` for DOT (TD maps to TB)." },
                    "max_depth":       { "type": "integer", "minimum": 0, "maximum": 100, "default": 0, "description": "Stop expanding below this nesting level; the elided subtree becomes one '… N keys/items hidden' node. Root is level 0. 0 (default) expands every level." },
                    "max_nodes":       { "type": "integer", "minimum": 1, "maximum": 5000, "default": 300, "description": "Hard cap on nodes in the diagram. Nodes are added breadth-first, so the cap keeps the upper levels; a truncation comment is appended when it bites. Default 300." },
                    "max_array_items": { "type": "integer", "minimum": 0, "maximum": 1000, "default": 0, "description": "Max elements drawn per array; the remainder collapses into one '… N more items' node. 0 (default) draws every element." },
                    "include_values":  { "type": "boolean", "default": true, "description": "Show scalar values in leaf labels ('name: \"Ada\"'). Set false for a keys-only structure map. Default true." },
                    "value_max_len":   { "type": "integer", "minimum": 0, "maximum": 200, "default": 40, "description": "Truncate key names and string values longer than this many characters, appending '…'. 0 disables truncation. Default 40." },
                    "show_types":      { "type": "boolean", "default": false, "description": "Annotate labels with type/size: objects get '{n}' keys, arrays '[n]' items, and (when include_values is false) scalars get their JSON type. Default false." }
                },
                "required": ["json"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn run_renders_mermaid_by_default() {
        let out = run(args(r#"{"a":[1]}"#)).unwrap();
        assert!(out.starts_with("flowchart TD\n"));
        assert!(out.contains("n1[[\"a\"]]"));
        assert!(out.contains("n2(\"[0]: 1\")"));
    }

    #[test]
    fn run_renders_dot_when_asked() {
        let mut a = args(r#"{"a":1}"#);
        a.format = "dot".into();
        a.direction = "LR".into();
        let out = run(a).unwrap();
        assert!(out.contains("digraph json {"));
        assert!(out.contains("rankdir=\"LR\";"));
    }

    #[test]
    fn run_rejects_invalid_json() {
        assert!(run(args("{oops")).is_err());
    }
}
