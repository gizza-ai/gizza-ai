//! Browser-facing wasm-bindgen wrapper for /tools/rncryptor-encrypt/.
//! The argument order mirrors `page/meta.toml`'s `[[input]]` order.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    operation: &str,
    data: &str,
    password: &str,
    data_encoding: &str,
    output_encoding: &str,
    encryption_salt: &str,
    hmac_salt: &str,
    iv: &str,
) -> Result<String, JsValue> {
    gizza_ai_rncryptor_encrypt_core::run(
        operation,
        data,
        password,
        data_encoding,
        output_encoding,
        encryption_salt,
        hmac_salt,
        iv,
    )
    .map_err(|e| JsValue::from_str(&e))
}
