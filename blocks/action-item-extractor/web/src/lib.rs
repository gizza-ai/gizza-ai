//! Browser-facing wasm-bindgen wrapper for /tools/action-item-extractor/.
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    !matches!(s.trim().to_ascii_lowercase().as_str(), "false" | "0" | "off" | "no")
}

#[wasm_bindgen]
pub fn run(input: &str, format: &str, group_by: &str, include_decisions: &str) -> Result<String, JsValue> {
    let format = if format.trim().is_empty() { "markdown" } else { format };
    let group_by = if group_by.trim().is_empty() { "type" } else { group_by };
    gizza_ai_action_item_extractor_core::extract(input, format, group_by, truthy(include_decisions))
        .map_err(|e| JsValue::from_str(&e))
}
