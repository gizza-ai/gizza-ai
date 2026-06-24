//! Browser-facing wasm-bindgen wrapper for /tools/pgp-verify/.
//! Field order MUST match meta.toml: signature, public_key, message.
use gizza_ai_pgp_verify_core::run as verify;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(signature: &str, public_key: &str, message: &str) -> Result<String, JsValue> {
    let res = verify(message, signature, public_key).map_err(|e| JsValue::from_str(&e))?;
    serde_json::to_string_pretty(&res.to_json()).map_err(|e| JsValue::from_str(&e.to_string()))
}
