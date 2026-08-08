//! Browser-facing wasm-bindgen wrapper for /tools/yaml-path-query/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    yaml: &str,
    path: &str,
    mode: &str,
    value: &str,
    format: &str,
) -> Result<String, JsValue> {
    gizza_ai_yaml_path_query_core::run(yaml, path, mode, value, format)
        .map_err(|e| JsValue::from_str(&e))
}
