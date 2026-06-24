//! Browser-facing wasm-bindgen wrapper for /tools/pem-public-key-extract/.
//! Field order MUST match meta.toml: input, key_type, der_format.
use gizza_ai_pem_public_key_extract_core::{parse_der_format, parse_key_type, run};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn extract_public_key(
    input: &str,
    key_type: &str,
    der_format: &str,
) -> Result<String, JsValue> {
    let kt = parse_key_type(key_type).map_err(|e| JsValue::from_str(&e))?;
    let fmt = parse_der_format(der_format).map_err(|e| JsValue::from_str(&e))?;
    run(input, kt, fmt).map_err(|e| JsValue::from_str(&e))
}
