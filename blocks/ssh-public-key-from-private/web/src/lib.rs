//! Browser-facing wasm-bindgen wrapper for /tools/ssh-public-key-from-private/.
//! Field order MUST match meta.toml: input, key_type, der_format, comment.
use gizza_ai_ssh_public_key_from_private_core::{parse_der_format, parse_key_type, run_with};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn ssh_public_key(
    input: &str,
    key_type: &str,
    der_format: &str,
    comment: &str,
) -> Result<String, JsValue> {
    let kt = parse_key_type(key_type).map_err(|e| JsValue::from_str(&e))?;
    let fmt = parse_der_format(der_format).map_err(|e| JsValue::from_str(&e))?;
    run_with(input, kt, fmt, comment).map_err(|e| JsValue::from_str(&e))
}
