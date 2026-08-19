//! Browser-facing wasm-bindgen wrapper for /tools/registry-hive-parser/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    data: &str,
    input_encoding: &str,
    mode: &str,
    path: &str,
    max_entries: usize,
) -> Result<String, JsValue> {
    gizza_ai_registry_hive_parser_core::run(data, input_encoding, mode, path, max_entries)
        .map_err(|e| JsValue::from_str(&e))
}
