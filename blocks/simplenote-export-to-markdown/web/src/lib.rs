//! Browser-facing wasm-bindgen wrapper for /tools/simplenote-export-to-markdown/.
//!
//! The standalone page passes every field value as a string, so the boolean
//! `include_trashed` arrives as a string and is parsed here.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    input: &str,
    filename_style: &str,
    metadata: &str,
    include_trashed: &str,
) -> Result<String, JsValue> {
    let include_trashed = matches!(
        include_trashed.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    );
    gizza_ai_simplenote_export_to_markdown_core::convert(
        input,
        filename_style,
        metadata,
        include_trashed,
    )
    .map_err(|e| JsValue::from_str(&e))
}
