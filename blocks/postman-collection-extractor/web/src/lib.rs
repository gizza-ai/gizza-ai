//! Browser-facing wasm-bindgen wrapper for /tools/postman-collection-extractor/.
//! Field order MUST match meta.toml: collection, format, method, url_contains,
//! folder, variables, resolve_variables. Fields arrive as strings; an empty
//! select falls back to the descriptor default (deep-links may omit any
//! optional param).
use gizza_ai_postman_collection_extractor_core::extract;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    collection: &str,
    format: &str,
    method: &str,
    url_contains: &str,
    folder: &str,
    variables: &str,
    resolve_variables: &str,
) -> Result<String, JsValue> {
    let format = if format.trim().is_empty() {
        "list"
    } else {
        format
    };
    // Checkboxes arrive as "true"/"false"; treat every positive form as on and
    // an empty value as the descriptor default (true).
    let resolve = match resolve_variables.trim() {
        "" => true,
        v => matches!(v, "true" | "1" | "on" | "yes"),
    };
    extract(collection, format, method, url_contains, folder, variables, resolve)
        .map_err(|e| JsValue::from_str(&e))
}
