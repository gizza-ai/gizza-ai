//! Browser-facing wasm-bindgen wrapper for /tools/kanban-board-builder/.
use gizza_ai_kanban_board_builder_core::{build, parse_wip_limit};
use wasm_bindgen::prelude::*;

fn truthy(v: &str) -> bool {
    matches!(v.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

fn or_default<'a>(v: &'a str, fallback: &'a str) -> &'a str {
    if v.trim().is_empty() {
        fallback
    } else {
        v
    }
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    tasks: &str,
    columns: &str,
    format: &str,
    title: &str,
    default_column: &str,
    wip_limit: &str,
    sort_by: &str,
    show_counts: &str,
) -> Result<String, JsValue> {
    let wip = parse_wip_limit(wip_limit).map_err(|e| JsValue::from_str(&e))?;
    build(
        tasks,
        columns,
        or_default(format, "markdown"),
        title,
        default_column,
        wip,
        or_default(sort_by, "none"),
        truthy(show_counts),
    )
    .map_err(|e| JsValue::from_str(&e))
}
