//! Browser-facing wasm-bindgen wrapper for /tools/jsonata-query/.
//! Field order MUST match meta.toml: query, json, pretty.
use gizza_ai_jsonata_query_core::run_jsonata;
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
pub fn run(query: &str, json: &str, pretty: &str) -> Result<String, JsValue> {
    run_jsonata(query, json, truthy(pretty)).map_err(|e| JsValue::from_str(&e))
}
