//! Browser-facing wasm-bindgen wrapper for /tools/document-splitter/.
use gizza_ai_document_splitter_core::{preview, Format};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(document: &str, format: &str) -> Result<String, JsValue> {
    let fmt = Format::parse(format).map_err(|e| JsValue::from_str(&e))?;
    preview(document, fmt).map_err(|e| JsValue::from_str(&e))
}
