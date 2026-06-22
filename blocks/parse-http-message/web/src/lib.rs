//! Browser-facing wasm-bindgen wrapper for /tools/parse-http-message/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(message: &str) -> Result<String, JsValue> {
    gizza_ai_parse_http_message_core::render(message).map_err(|e| JsValue::from_str(&e))
}
