//! Browser-facing wasm-bindgen wrapper for /tools/bitcoin-address/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(key: &str, network: &str, compressed: &str) -> Result<String, JsValue> {
    let compressed = matches!(compressed, "" | "true" | "1" | "on" | "yes");
    gizza_ai_bitcoin_address_core::derive(key, network, compressed)
        .map_err(|e| JsValue::from_str(&e))
}
