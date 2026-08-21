//! Browser-facing wasm-bindgen wrapper for /tools/gantt-mermaid-generator/.
//! The page driver hands every field through as a raw string, so each argument
//! is parsed here and the core owns all validation.
use wasm_bindgen::prelude::*;

/// Parse a checkbox field; a blank value keeps the descriptor default.
fn flag(value: &str, default: bool) -> bool {
    let t = value.trim();
    if t.is_empty() {
        default
    } else {
        matches!(t, "true" | "1" | "on" | "yes")
    }
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    tasks: &str,
    title: &str,
    delimiter: &str,
    date_format: &str,
    axis_format: &str,
    tick_interval: &str,
    exclude_weekends: &str,
    weekend: &str,
    excludes: &str,
    today_marker: &str,
    compact: &str,
    fence: &str,
) -> Result<String, JsValue> {
    gizza_ai_gantt_mermaid_generator_core::generate(
        tasks,
        title,
        delimiter,
        date_format,
        axis_format,
        tick_interval,
        flag(exclude_weekends, false),
        weekend,
        excludes,
        flag(today_marker, true),
        flag(compact, false),
        flag(fence, false),
    )
    .map_err(|e| JsValue::from_str(&e))
}
