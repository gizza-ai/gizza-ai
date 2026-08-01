//! Browser-facing wasm-bindgen wrapper for /tools/hmac-verify/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    message: &str,
    key: &str,
    expected: &str,
    algorithm: &str,
    message_encoding: &str,
    key_encoding: &str,
    expected_encoding: &str,
) -> Result<String, JsValue> {
    gizza_ai_hmac_verify_core::verify_report(
        message,
        key,
        expected,
        algorithm,
        message_encoding,
        key_encoding,
        expected_encoding,
    )
    .map_err(|e| JsValue::from_str(&e))
}
