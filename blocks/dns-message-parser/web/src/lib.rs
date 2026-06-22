//! Browser-facing wasm-bindgen wrapper for /tools/dns-message-parser/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(message: &str) -> Result<String, JsValue> {
    gizza_ai_dns_message_parser_core::render(message).map_err(|e| JsValue::from_str(&e))
}
