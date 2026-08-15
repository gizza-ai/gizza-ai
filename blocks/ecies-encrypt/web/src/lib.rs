//! Browser-facing wasm-bindgen wrapper for /tools/ecies-encrypt/.
use wasm_bindgen::prelude::*;

fn parse_bool(s: &str, default: bool) -> bool {
    match s.trim().to_ascii_lowercase().as_str() {
        "" => default,
        "true" | "1" | "yes" | "on" => true,
        "false" | "0" | "no" | "off" => false,
        _ => default,
    }
}

#[wasm_bindgen]
pub fn run(
    operation: &str,
    data: &str,
    key: &str,
    curve: &str,
    cipher: &str,
    nonce_length: &str,
    nonce: &str,
    compressed_ephemeral: &str,
    kdf_input: &str,
    ephemeral_key: &str,
    key_encoding: &str,
    data_encoding: &str,
    output_encoding: &str,
) -> Result<String, JsValue> {
    gizza_ai_ecies_encrypt_core::run(
        operation,
        data,
        key,
        curve,
        cipher,
        nonce_length,
        nonce,
        parse_bool(compressed_ephemeral, false),
        kdf_input,
        ephemeral_key,
        key_encoding,
        data_encoding,
        output_encoding,
    )
    .map_err(|e| JsValue::from_str(&e))
}
