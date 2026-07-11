//! Browser-facing wasm-bindgen wrapper for /tools/ed25519-sign-verify/.
//! Field order MUST match meta.toml: operation, message, message_encoding, key, signature.
use gizza_ai_ed25519_sign_verify_core::{process, MsgEncoding, Operation};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    operation: &str,
    message: &str,
    message_encoding: &str,
    key: &str,
    signature: &str,
) -> Result<String, JsValue> {
    let op = Operation::parse(operation).map_err(|e| JsValue::from_str(&e))?;
    let enc = MsgEncoding::parse(message_encoding).map_err(|e| JsValue::from_str(&e))?;
    let out = process(op, message, enc, key, signature).map_err(|e| JsValue::from_str(&e))?;
    let text = match op {
        Operation::Sign => format!(
            "operation: sign\nsignature (hex): {}\nsignature (base64): {}\nlength: {} bytes\npublic key (hex): {}\npublic key (base64): {}",
            out.signature_hex.unwrap_or_default(),
            out.signature_base64.unwrap_or_default(),
            out.signature_bytes.unwrap_or(0),
            out.public_key_hex.unwrap_or_default(),
            out.public_key_base64.unwrap_or_default(),
        ),
        Operation::Verify => {
            let valid = out.valid.unwrap_or(false);
            format!(
                "operation: verify\nvalid: {}\n{}",
                valid,
                if valid {
                    "✓ signature is valid for this message and public key"
                } else {
                    "✗ signature does NOT match this message and public key"
                }
            )
        }
    };
    Ok(text)
}
