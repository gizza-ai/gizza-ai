//! Browser-facing wasm-bindgen wrapper for /tools/bulk-artifact-extractor/.
//! Field order MUST match page/meta.toml: text, kinds, output, context, limit.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    text: &str,
    kinds: &str,
    output: &str,
    context: &str,
    limit: &str,
) -> Result<String, JsValue> {
    let context = context.trim().parse::<u32>().unwrap_or(24);
    let limit = limit.trim().parse::<u32>().unwrap_or(1000);
    gizza_ai_bulk_artifact_extractor_core::extract(text, kinds, output, context, limit)
        .map_err(|e| JsValue::from_str(&e))
}
