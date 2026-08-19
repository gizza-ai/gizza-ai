//! Browser-facing wasm-bindgen wrapper for /tools/nacl-box-encrypt/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    operation: &str,
    data: &str,
    recipient_key: &str,
    sender_key: &str,
    nonce: &str,
    key_encoding: &str,
    nonce_encoding: &str,
    data_encoding: &str,
    output_encoding: &str,
) -> Result<String, JsValue> {
    gizza_ai_nacl_box_encrypt_core::run(
        operation,
        data,
        recipient_key,
        sender_key,
        nonce,
        key_encoding,
        nonce_encoding,
        data_encoding,
        output_encoding,
    )
    .map_err(|e| JsValue::from_str(&e))
}
