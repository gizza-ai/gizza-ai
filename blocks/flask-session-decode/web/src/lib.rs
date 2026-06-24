//! Browser-facing wasm-bindgen wrapper for /tools/flask-session-decode/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(input: &str) -> Result<String, JsValue> {
    gizza_ai_flask_session_decode_core::decode_to_json(input).map_err(|e| JsValue::from_str(&e))
}
