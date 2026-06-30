//! Browser-facing wasm-bindgen wrapper for /tools/text-to-unicode/.
//! Field order MUST match meta.toml: text, format.
use gizza_ai_text_to_unicode_core::{to_unicode, Format};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(text: &str, format: &str) -> Result<String, JsValue> {
    let fmt = Format::parse(format).map_err(|e| JsValue::from_str(&e))?;
    to_unicode(text, fmt).map_err(|e| JsValue::from_str(&e))
}
