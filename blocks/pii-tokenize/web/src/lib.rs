//! Browser-facing wasm-bindgen wrapper for /tools/pii-tokenize/.
//! Field order MUST match meta.toml: text, secret, preserve_email_domain.
use gizza_ai_pii_tokenize_core::{parse_bool, tokenize};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(text: &str, secret: &str, preserve_email_domain: &str) -> Result<String, JsValue> {
    let preserve = parse_bool(preserve_email_domain).map_err(|e| JsValue::from_str(&e))?;
    Ok(tokenize(text, secret, preserve).tokenized)
}
