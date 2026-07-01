//! Browser-facing wasm-bindgen wrapper for /tools/xor-cipher/.
//! Field order MUST match page/meta.toml: data, input, key, key_format, output.

use gizza_ai_xor_cipher_core::{xor_cipher, DataFormat, OutFormat};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(data: &str, input: &str, key: &str, key_format: &str, output: &str) -> Result<String, JsValue> {
    let input = DataFormat::parse(input).map_err(|e| JsValue::from_str(&e))?;
    let key_format = DataFormat::parse(key_format).map_err(|e| JsValue::from_str(&e))?;
    let output = OutFormat::parse(output).map_err(|e| JsValue::from_str(&e))?;
    xor_cipher(data, input, key, key_format, output).map_err(|e| JsValue::from_str(&e))
}
