//! Browser-facing wasm-bindgen wrapper for /tools/json-merge/.
//! Field order MUST match meta.toml: documents, concat_arrays, indent.
use gizza_ai_json_merge_core::merge_documents;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(documents: &str, concat_arrays: &str, indent: &str) -> Result<String, JsValue> {
    let concat = matches!(
        concat_arrays.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    );
    let n: usize = indent.trim().parse().unwrap_or(2);
    merge_documents(documents, concat, n).map_err(|e| JsValue::from_str(&e))
}
