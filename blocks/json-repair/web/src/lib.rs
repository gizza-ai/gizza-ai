//! Browser-facing wasm-bindgen wrapper for /tools/json-repair/.
//! Field order MUST match page/meta.toml: json, indent. Fields arrive as strings.
use gizza_ai_json_repair_core::repair;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(json: &str, indent: &str) -> Result<String, JsValue> {
    let ind = indent.trim();
    let ind = if ind.is_empty() { "2" } else { ind };
    repair(json, ind).map_err(|e| JsValue::from_str(&e))
}
