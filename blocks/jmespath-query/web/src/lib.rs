//! Browser-facing wasm-bindgen wrapper for /tools/jmespath-query/.
//! Field order MUST match meta.toml: expression, json, pretty, raw.
use gizza_ai_jmespath_query_core::run_jmespath;
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

#[wasm_bindgen]
pub fn run(expression: &str, json: &str, pretty: &str, raw: &str) -> Result<String, JsValue> {
    run_jmespath(expression, json, truthy(pretty), truthy(raw)).map_err(|e| JsValue::from_str(&e))
}
