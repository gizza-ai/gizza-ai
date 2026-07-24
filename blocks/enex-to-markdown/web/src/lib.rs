//! Browser-facing wasm-bindgen wrapper for /tools/enex-to-markdown/.
//! Field order MUST match meta.toml: enex, format, metadata, attachments.
//! Fields arrive as strings (checkbox as "true"/"false").
use gizza_ai_enex_to_markdown_core::{convert, Format, Metadata, Options};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(enex: &str, format: &str, metadata: &str, attachments: &str) -> Result<String, JsValue> {
    let opts = Options {
        format: Format::parse(format).map_err(|e| JsValue::from_str(&e))?,
        metadata: Metadata::parse(metadata).map_err(|e| JsValue::from_str(&e))?,
        attachments: matches!(attachments.trim(), "true" | "1" | "on" | "yes"),
    };
    convert(enex, opts)
        .map(|c| c.content)
        .map_err(|e| JsValue::from_str(&e))
}
