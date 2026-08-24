//! Browser-facing wasm-bindgen wrapper for /tools/eth-address-from-key/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(key: &str, key_type: &str, output_format: &str) -> Result<String, JsValue> {
    gizza_ai_eth_address_from_key_core::run(key, key_type, output_format)
        .map_err(|e| JsValue::from_str(&e))
}
