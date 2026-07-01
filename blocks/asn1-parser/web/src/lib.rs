//! Browser-facing wasm-bindgen wrapper for /tools/asn1-parser/.
//! Field order MUST match meta.toml: input, format.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(input: &str, format: &str) -> Result<String, JsValue> {
    let format = if format.trim().is_empty() { "tree" } else { format };
    gizza_ai_asn1_parser_core::parse(input, format).map_err(|e| JsValue::from_str(&e))
}
