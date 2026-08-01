//! Browser-facing wasm-bindgen wrapper for /tools/nostr-key-encoder/.
//! Param order MUST match the field order in page/meta.toml.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    input: &str,
    mode: &str,
    r#type: &str,
    relays: &str,
    author: &str,
    kind: f64,
) -> Result<String, JsValue> {
    gizza_ai_nostr_key_encoder_core::convert(input, mode, r#type, relays, author, kind as i64)
        .map_err(|e| JsValue::from_str(&e))
}
