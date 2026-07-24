//! Browser-facing wasm-bindgen wrapper for /tools/json-from-logs/.
//! Field order MUST match meta.toml: text, indent, output. Fields arrive as strings.
use gizza_ai_json_from_logs_core::run as core_run;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(text: &str, indent: &str, output: &str) -> Result<String, JsValue> {
    let n: usize = indent.trim().parse().unwrap_or(2);
    core_run(text, n, output).map_err(|e| JsValue::from_str(&e))
}
