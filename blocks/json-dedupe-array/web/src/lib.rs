//! Browser-facing wasm-bindgen wrapper for /tools/json-dedupe-array/.
//! Field order MUST match meta.toml: json, keys, root, keep, ignore_case,
//! output, indent. Fields arrive as strings; checkboxes as "true"/"false".
use wasm_bindgen::prelude::*;

fn truthy(v: &str) -> bool {
    matches!(v.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
pub fn run(
    json: &str,
    keys: &str,
    root: &str,
    keep: &str,
    ignore_case: &str,
    output: &str,
    indent: &str,
) -> Result<String, JsValue> {
    gizza_ai_json_dedupe_array_core::run(
        json,
        keys,
        root,
        keep,
        truthy(ignore_case),
        output,
        indent,
    )
    .map_err(|e| JsValue::from_str(&e))
}
