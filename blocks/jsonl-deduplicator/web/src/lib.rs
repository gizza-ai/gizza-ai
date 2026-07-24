//! Browser-facing wasm-bindgen wrapper for /tools/jsonl-deduplicator/.
//! Field order MUST match meta.toml: data, keys, keep, ignore_case, on_invalid.
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
pub fn run(
    data: &str,
    keys: &str,
    keep: &str,
    ignore_case: &str,
    on_invalid: &str,
) -> Result<String, JsValue> {
    gizza_ai_jsonl_deduplicator_core::run(data, keys, keep, truthy(ignore_case), on_invalid)
        .map_err(|e| JsValue::from_str(&e))
}
