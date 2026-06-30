//! Browser-facing wasm-bindgen wrapper for /tools/pgp-key-info/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(key: &str) -> Result<String, JsValue> {
    gizza_ai_pgp_key_info_core::run(key).map_err(|e| JsValue::from_str(&e))
}
