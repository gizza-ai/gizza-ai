//! Browser-facing wasm-bindgen wrapper for /tools/elasticsearch-bulk-formatter/.
//! The generic page passes every field value as a string, so parse the checkbox
//! here and delegate all formatting/validation to the pure core.
use wasm_bindgen::prelude::*;

fn truthy(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

#[wasm_bindgen]
pub fn run(
    documents: &str,
    action: &str,
    index: &str,
    id_field: &str,
    doc_as_upsert: &str,
) -> Result<String, JsValue> {
    gizza_ai_elasticsearch_bulk_formatter_core::run(
        documents,
        action,
        index,
        id_field,
        truthy(doc_as_upsert),
    )
    .map_err(|e| JsValue::from_str(&e))
}
