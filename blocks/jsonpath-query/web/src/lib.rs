//! Browser-facing wasm-bindgen wrapper for /tools/jsonpath-query/.
//! Field order MUST match meta.toml: path, json, pretty, wrap.
use gizza_ai_jsonpath_query_core::query_jsonpath;
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
pub fn run(path: &str, json: &str, pretty: &str, wrap: &str) -> Result<String, JsValue> {
    let outs = query_jsonpath(path, json, truthy(pretty), truthy(wrap)).map_err(|e| JsValue::from_str(&e))?;
    Ok(outs.join("\n"))
}
