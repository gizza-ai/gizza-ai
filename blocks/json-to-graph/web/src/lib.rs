//! Browser-facing wasm-bindgen wrapper for /tools/json-to-graph/.
//! Field order MUST match meta.toml: json, format, direction, max_depth,
//! max_nodes, max_array_items, include_values, value_max_len, show_types.
use gizza_ai_json_to_graph_core::{to_graph, Direction, Format, Options};
use wasm_bindgen::prelude::*;

fn parse_num(s: &str, default: usize) -> usize {
    match s.trim() {
        "" => default,
        t => t.parse::<usize>().unwrap_or(default),
    }
}

fn parse_bool(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    json: &str,
    format: &str,
    direction: &str,
    max_depth: &str,
    max_nodes: &str,
    max_array_items: &str,
    include_values: &str,
    value_max_len: &str,
    show_types: &str,
) -> Result<String, JsValue> {
    let opts = Options {
        format: Format::parse(format),
        direction: Direction::parse(direction),
        max_depth: parse_num(max_depth, 0),
        max_nodes: parse_num(max_nodes, 300),
        max_array_items: parse_num(max_array_items, 0),
        include_values: parse_bool(include_values),
        value_max_len: parse_num(value_max_len, 40),
        show_types: parse_bool(show_types),
    };
    to_graph(json, &opts).map_err(|e| JsValue::from_str(&e))
}
