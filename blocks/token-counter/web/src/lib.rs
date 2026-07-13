//! Browser-facing wasm-bindgen wrapper for /tools/token-counter/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(text: &str, model: &str) -> Result<String, JsValue> {
    gizza_ai_token_counter_core::count(text, model).map_err(|e| JsValue::from_str(&e))
}
