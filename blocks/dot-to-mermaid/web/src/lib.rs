//! Browser-facing wasm-bindgen wrapper for /tools/dot-to-mermaid/.
//! The page driver hands every field through as a string, so each flag is
//! parsed here (blank = the descriptor default) and the core owns validation.
use gizza_ai_dot_to_mermaid_core::{convert, Options};
use wasm_bindgen::prelude::*;

/// Positive-truthy parse with an explicit default for a blank field.
fn flag(v: &str, default: bool) -> bool {
    match v.trim() {
        "" => default,
        s => matches!(
            s.to_ascii_lowercase().as_str(),
            "true" | "1" | "on" | "yes"
        ),
    }
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    dot: &str,
    direction: &str,
    shapes: &str,
    edge_labels: &str,
    link_styles: &str,
    subgraphs: &str,
    colors: &str,
    warnings: &str,
    title: &str,
    fence: &str,
) -> Result<String, JsValue> {
    let opts = Options {
        direction: if direction.trim().is_empty() {
            "auto".to_string()
        } else {
            direction.trim().to_string()
        },
        shapes: flag(shapes, true),
        edge_labels: flag(edge_labels, true),
        link_styles: flag(link_styles, true),
        subgraphs: flag(subgraphs, true),
        colors: flag(colors, true),
        warnings: flag(warnings, true),
        title: title.to_string(),
        fence: flag(fence, false),
    };
    convert(dot, &opts).map_err(|e| JsValue::from_str(&e))
}
