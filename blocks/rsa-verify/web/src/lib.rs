//! Browser-facing wasm-bindgen wrapper for /tools/rsa-verify/.
//! Field order MUST match meta.toml: message, signature, public_key, scheme, hash.
use gizza_ai_rsa_verify_core::{verify, Hash, Scheme};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    message: &str,
    signature: &str,
    public_key: &str,
    scheme: &str,
    hash: &str,
) -> Result<String, JsValue> {
    let scheme = Scheme::parse(scheme).map_err(|e| JsValue::from_str(&e))?;
    let hash = Hash::parse(hash).map_err(|e| JsValue::from_str(&e))?;
    let valid = verify(message, signature, public_key, scheme, hash)
        .map_err(|e| JsValue::from_str(&e))?;
    Ok(if valid {
        "VALID — the signature is authentic for this message and public key.".to_string()
    } else {
        "INVALID — the signature does not verify for this message, key, scheme, and hash.".to_string()
    })
}
