//! Browser-facing wasm-bindgen wrapper for /tools/scrypt-derive/.
//! Field order MUST match meta.toml: password, mode, salt, salt_encoding,
//! n, r, p, length, encoding, expected.
use gizza_ai_scrypt_derive_core::{derive, verify};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    password: &str,
    mode: &str,
    salt: &str,
    salt_encoding: &str,
    n: &str,
    r: &str,
    p: &str,
    length: &str,
    encoding: &str,
    expected: &str,
) -> Result<String, JsValue> {
    let n_val: u32 = n.trim().parse().unwrap_or(16384);
    let r_val: u32 = r.trim().parse().unwrap_or(8);
    let p_val: u32 = p.trim().parse().unwrap_or(1);
    let len: usize = length.trim().parse().unwrap_or(32);
    let salt_enc = if salt_encoding.trim().is_empty() {
        "utf8"
    } else {
        salt_encoding
    };
    let enc = if encoding.trim().is_empty() {
        "hex"
    } else {
        encoding
    };
    match mode.trim().to_ascii_lowercase().as_str() {
        "verify" => {
            if expected.trim().is_empty() {
                return Err(JsValue::from_str("verify mode requires the expected key"));
            }
            let m = verify(password, salt, salt_enc, n_val, r_val, p_val, expected)
                .map_err(|e| JsValue::from_str(&e))?;
            Ok(if m {
                "✓ match".to_string()
            } else {
                "✗ no match".to_string()
            })
        }
        _ => derive(password, salt, salt_enc, n_val, r_val, p_val, len, enc)
            .map_err(|e| JsValue::from_str(&e)),
    }
}
