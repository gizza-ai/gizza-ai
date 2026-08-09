//! Browser-facing wasm-bindgen wrapper for /tools/yaml-deep-merge/.
//! Field order MUST match meta.toml: documents, precedence, object_merge,
//! array_merge, array_key, null_deletes, sort_keys, indent.
use gizza_ai_yaml_deep_merge_core::merge_documents;
use wasm_bindgen::prelude::*;

fn truthy(v: &str) -> bool {
    matches!(v.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    documents: &str,
    precedence: &str,
    object_merge: &str,
    array_merge: &str,
    array_key: &str,
    null_deletes: &str,
    sort_keys: &str,
    indent: &str,
) -> Result<String, JsValue> {
    let n: usize = match indent.trim() {
        "" => 2,
        raw => raw
            .parse()
            .map_err(|_| JsValue::from_str(&format!("indent must be a whole number 1-8, got '{raw}'")))?,
    };
    merge_documents(
        documents,
        precedence,
        object_merge,
        array_merge,
        array_key,
        truthy(null_deletes),
        truthy(sort_keys),
        n,
    )
    .map_err(|e| JsValue::from_str(&e))
}
