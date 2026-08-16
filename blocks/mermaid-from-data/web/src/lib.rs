//! Browser-facing wasm-bindgen wrapper for /tools/mermaid-from-data/.
use wasm_bindgen::prelude::*;

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    edges: &str,
    nodes: &str,
    diagram: &str,
    direction: &str,
    row_mode: &str,
    delimiter: &str,
    node_shape: &str,
    edge_style: &str,
    title: &str,
    fence: &str,
) -> Result<String, JsValue> {
    let fence = matches!(
        fence.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    );
    gizza_ai_mermaid_from_data_core::generate(
        edges, nodes, diagram, direction, row_mode, delimiter, node_shape, edge_style, title, fence,
    )
    .map_err(|e| JsValue::from_str(&e))
}
