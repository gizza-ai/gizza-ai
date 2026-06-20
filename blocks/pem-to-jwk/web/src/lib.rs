//! Browser-facing wasm-bindgen wrapper for /tools/pem-to-jwk/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(input: &str) -> Result<String, JsValue> {
    let jwk = gizza_ai_pem_to_jwk_core::run(input).map_err(|e| JsValue::from_str(&e))?;
    serde_json::to_string_pretty(&jwk).map_err(|e| JsValue::from_str(&e.to_string()))
}
