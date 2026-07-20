//! Browser-facing wasm-bindgen wrapper for /tools/postman-collection-converter/.
//! Field order MUST match meta.toml: collection, target, variables, multiline.
//! Fields arrive as strings (checkboxes send "true"/"false").
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

#[wasm_bindgen]
pub fn run(
    collection: &str,
    target: &str,
    variables: &str,
    multiline: &str,
) -> Result<String, JsValue> {
    gizza_ai_postman_collection_converter_core::convert(
        collection,
        target,
        variables,
        truthy(multiline),
    )
    .map_err(|e| JsValue::from_str(&e))
}
