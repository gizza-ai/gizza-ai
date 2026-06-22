//! Browser-facing wasm-bindgen wrapper for /tools/ecdsa-sign/.
//! Field order MUST match meta.toml: message, private_key, curve, format.
use gizza_ai_ecdsa_sign_core::{sign, Curve, SigFormat};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(message: &str, private_key: &str, curve: &str, format: &str) -> Result<String, JsValue> {
    let curve = Curve::parse(curve).map_err(|e| JsValue::from_str(&e))?;
    let format = SigFormat::parse(format).map_err(|e| JsValue::from_str(&e))?;
    let s = sign(message, private_key, curve, format).map_err(|e| JsValue::from_str(&e))?;
    Ok(format!(
        "signature (base64): {}\nsignature (hex): {}\ncurve: {}\nformat: {}\nlength: {} bytes",
        s.signature_base64, s.signature_hex, s.curve, s.format, s.bytes
    ))
}
