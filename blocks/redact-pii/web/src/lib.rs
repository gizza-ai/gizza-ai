//! Browser-facing wasm-bindgen wrapper for /tools/redact-pii/.
//! Field order MUST match meta.toml: text, style.
use gizza_ai_redact_pii_core::{redact, Style};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(text: &str, style: &str) -> Result<String, JsValue> {
    let style = Style::parse(style).map_err(|e| JsValue::from_str(&e))?;
    Ok(redact(text, style).redacted)
}
