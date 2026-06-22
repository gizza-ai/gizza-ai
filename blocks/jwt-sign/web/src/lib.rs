//! Browser-facing wasm-bindgen wrapper for /tools/jwt-sign/.
//! Field order MUST match meta.toml: payload, secret, algorithm, header.
use gizza_ai_jwt_sign_core::{sign, Alg};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(payload: &str, secret: &str, algorithm: &str, header: &str) -> Result<String, JsValue> {
    let alg = Alg::parse(algorithm).map_err(|e| JsValue::from_str(&e))?;
    sign(header, payload, secret, alg).map_err(|e| JsValue::from_str(&e))
}
