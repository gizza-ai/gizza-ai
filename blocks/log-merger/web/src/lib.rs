//! Browser-facing wasm-bindgen wrapper for /tools/log-merger/.
//! Field order MUST match meta.toml: logs, source_mode, order, dedupe, align.
//! Fields arrive as strings (checkboxes send "true"/"false").
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
pub fn run(
    logs: &str,
    source_mode: &str,
    order: &str,
    dedupe: &str,
    align: &str,
) -> Result<String, JsValue> {
    // Empty enum fields fall back to the schema defaults.
    let source_mode = if source_mode.trim().is_empty() { "header" } else { source_mode };
    let order = if order.trim().is_empty() { "asc" } else { order };
    gizza_ai_log_merger_core::merge(logs, source_mode, order, truthy(dedupe), truthy(align))
        .map_err(|e| JsValue::from_str(&e))
}
