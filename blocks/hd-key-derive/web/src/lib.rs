//! Browser-facing wasm-bindgen wrapper for /tools/hd-key-derive/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    seed: &str,
    xprv: &str,
    path: &str,
    network: &str,
    address_type: &str,
) -> Result<String, JsValue> {
    gizza_ai_hd_key_derive_core::derive(seed, xprv, path, network, address_type)
        .map_err(|e| JsValue::from_str(&e))
}
