//! Browser-facing wasm-bindgen wrapper for /tools/json-to-typescript/.
//! Field order MUST match meta.toml: json, root_name, export.
use gizza_ai_json_to_typescript_core::infer;
use wasm_bindgen::prelude::*;

fn truthy_default_true(s: &str) -> bool {
    !matches!(s.trim().to_ascii_lowercase().as_str(), "false" | "0" | "off" | "no")
}

#[wasm_bindgen]
pub fn run(json: &str, root_name: &str, export: &str) -> Result<String, JsValue> {
    let root = if root_name.trim().is_empty() { "Root" } else { root_name };
    infer(json, root, truthy_default_true(export)).map_err(|e| JsValue::from_str(&e))
}
