//! Browser-facing wasm-bindgen wrapper for /tools/keep-to-markdown/.
//!
//! The standalone page passes every field value as a string, so the three
//! booleans arrive as `"true"` / `"false"` and are parsed here.
use wasm_bindgen::prelude::*;

fn flag(value: &str, default: bool) -> bool {
    match value.trim().to_ascii_lowercase().as_str() {
        "" => default,
        v => matches!(v, "true" | "1" | "yes" | "on"),
    }
}

#[wasm_bindgen]
pub fn run(
    input: &str,
    metadata: &str,
    filename_style: &str,
    checkbox_style: &str,
    include_archived: &str,
    include_trashed: &str,
    link_attachments: &str,
) -> Result<String, JsValue> {
    gizza_ai_keep_to_markdown_core::convert(
        input,
        metadata,
        filename_style,
        checkbox_style,
        flag(include_archived, true),
        flag(include_trashed, false),
        flag(link_attachments, true),
    )
    .map_err(|e| JsValue::from_str(&e))
}
