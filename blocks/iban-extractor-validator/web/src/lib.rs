//! Browser-facing wasm-bindgen wrapper for /tools/iban-extractor-validator/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(text: &str) -> Result<String, JsValue> {
    gizza_ai_iban_extractor_validator_core::render(text).map_err(|e| JsValue::from_str(&e))
}
