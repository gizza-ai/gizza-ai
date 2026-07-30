//! Browser-facing wasm-bindgen wrapper for /tools/cert-chain-validate/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(chain_pem: &str) -> Result<String, JsValue> {
    let now_unix = (js_sys::Date::now() / 1000.0) as i64;
    gizza_ai_cert_chain_validate_core::run_at(chain_pem, now_unix)
        .map_err(|e| JsValue::from_str(&e))
}
