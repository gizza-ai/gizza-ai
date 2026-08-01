//! Browser-facing wasm-bindgen wrapper for /tools/address-parse/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(address: &str, country: &str) -> Result<String, JsValue> {
    gizza_ai_address_parse_core::render(address, country).map_err(|e| JsValue::from_str(&e))
}
